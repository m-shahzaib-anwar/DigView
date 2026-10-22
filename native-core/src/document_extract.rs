//! Isolated document parser protocol. No paths, network, Word automation or macros.
use crate::db::Result;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read, Write};

pub const INPUT_LIMIT: usize = 16 * 1024 * 1024;
pub const TEXT_READ_LIMIT: usize = 2 * 1024 * 1024;
pub const OUTPUT_LIMIT: usize = 512 * 1024;
pub const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "text", "md", "markdown", "log", "csv", "tsv", "json", "jsonl", "xml", "yaml", "yml",
    "toml", "ini", "cfg", "conf", "html", "htm", "css", "scss", "less", "js", "mjs", "cjs", "jsx",
    "ts", "tsx", "rs", "py", "java", "c", "h", "cpp", "hpp", "cs", "go", "sql", "sh", "ps1", "bat",
    "cmd", "srt", "vtt", "rb", "php", "swift", "kt", "kts", "dart", "vue", "svelte", "rst",
    "ipynb",
];
pub const TEXT_NAMES: &[&str] = &[
    "readme",
    "license",
    "licence",
    "notice",
    "changelog",
    "makefile",
    "dockerfile",
];

pub fn kind(extension: &str, name: &str) -> Option<u8> {
    match extension.to_ascii_lowercase().as_str() {
        "pdf" => Some(b'p'),
        "docx" => Some(b'd'),
        e if TEXT_EXTENSIONS.contains(&e)
            || (e.is_empty() && TEXT_NAMES.contains(&name.to_ascii_lowercase().as_str())) =>
        {
            Some(b't')
        }
        _ => None,
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Extracted {
    pub text: String,
    pub truncated: bool,
}
fn bound(mut text: String, mut truncated: bool) -> Extracted {
    if text.len() > OUTPUT_LIMIT {
        let mut end = OUTPUT_LIMIT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        truncated = true;
    }
    Extracted { text, truncated }
}

struct CappedWriter {
    bytes: Vec<u8>,
    full: bool,
}
impl Write for CappedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len() + bytes.len() > OUTPUT_LIMIT {
            self.full = true;
            return Err(std::io::Error::other("text limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn extract(kind: u8, bytes: &[u8]) -> Result<Extracted> {
    if bytes.len() > INPUT_LIMIT {
        return Err("too_large".into());
    }
    match kind {
        b't' => {
            let read = bytes.len().min(TEXT_READ_LIMIT);
            let (text, _) = crate::content_preview::decode_text(
                &bytes[..read],
                bytes.len() >= TEXT_READ_LIMIT,
            )?;
            Ok(bound(text, bytes.len() >= TEXT_READ_LIMIT))
        }
        b'p' => {
            if !bytes.starts_with(b"%PDF-") {
                return Err("invalid_pdf".into());
            }
            let doc = pdf_extract::Document::load_mem(bytes).map_err(|_| "invalid_pdf")?;
            if doc.is_encrypted() {
                return Err("encrypted_pdf".into());
            }
            let pages = doc.get_pages();
            let mut writer = CappedWriter {
                bytes: vec![],
                full: false,
            };
            let mut truncated = pages.len() > 200;
            for page in pages.keys().take(200) {
                let result = {
                    let mut output =
                        pdf_extract::PlainTextOutput::new(&mut writer as &mut dyn Write);
                    pdf_extract::output_doc_page(&doc, &mut output, *page)
                };
                if writer.full {
                    truncated = true;
                    break;
                }
                result.map_err(|_| "invalid_pdf")?;
                if writer.write_all(b"\n").is_err() {
                    truncated = true;
                    break;
                }
            }
            let text = String::from_utf8(writer.bytes).map_err(|_| "invalid_pdf_text")?;
            Ok(bound(text, truncated))
        }
        b'd' => docx(bytes),
        _ => Err("unsupported".into()),
    }
}

fn docx(bytes: &[u8]) -> Result<Extracted> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "invalid_docx")?;
    if archive.len() > 2048 {
        return Err("docx_limit".into());
    }
    let mut parts = archive
        .file_names()
        .filter(|name| {
            matches!(
                *name,
                "word/document.xml" | "word/footnotes.xml" | "word/endnotes.xml"
            ) || ((name.starts_with("word/header") || name.starts_with("word/footer"))
                && name.ends_with(".xml"))
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if !parts.iter().any(|s| s == "word/document.xml") || parts.len() > 32 {
        return Err("invalid_docx".into());
    }
    parts.sort_by_key(|name| (name != "word/document.xml", name.clone()));
    let mut text = String::new();
    let mut total = 0usize;
    for part in parts {
        let entry = archive.by_name(&part).map_err(|_| "invalid_docx")?;
        if entry.size() > 4 * 1024 * 1024 {
            return Err("docx_limit".into());
        }
        let mut xml = vec![];
        entry
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut xml)
            .map_err(|_| "invalid_docx")?;
        total += xml.len();
        if xml.len() > 4 * 1024 * 1024 || total > 8 * 1024 * 1024 {
            return Err("docx_limit".into());
        }
        if xml_text(&xml, &mut text)? {
            return Ok(bound(text, true));
        }
        text.push('\n');
    }
    Ok(bound(text, false))
}
fn xml_text(xml: &[u8], text: &mut String) -> Result<bool> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_reader(xml);
    let mut in_text = false;
    loop {
        match reader.read_event().map_err(|_| "invalid_docx_xml")? {
            Event::DocType(_) => return Err("docx_entities_unsupported".into()),
            Event::Start(e) => {
                if e.local_name().as_ref() == b"t" {
                    in_text = true;
                }
            }
            Event::End(e) => match e.local_name().as_ref() {
                b"t" => in_text = false,
                b"p" | b"tr" => text.push('\n'),
                _ => {}
            },
            Event::Empty(e) => match e.local_name().as_ref() {
                b"tab" => text.push('\t'),
                b"br" | b"cr" => text.push('\n'),
                _ => {}
            },
            Event::Text(e) if in_text => {
                text.push_str(&e.decode().map_err(|_| "invalid_docx_text")?)
            }
            Event::CData(e) if in_text => {
                text.push_str(&e.decode().map_err(|_| "invalid_docx_text")?)
            }
            Event::GeneralRef(e) if in_text => {
                let reference = e.decode().map_err(|_| "invalid_docx_text")?;
                let escaped = format!("&{reference};");
                text.push_str(
                    &quick_xml::escape::unescape(&escaped)
                        .map_err(|_| "docx_entities_unsupported")?,
                );
            }
            Event::Eof => break,
            _ => {}
        }
        if text.len() >= OUTPUT_LIMIT {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Called before the desktop runtime. Input is bounded bytes on stdin, never a path.
pub fn worker_main_if_requested() -> bool {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) != Some("--digview-content-worker") {
        return false;
    }
    let result = std::panic::catch_unwind(|| -> Result<Extracted> {
        let kind = args
            .get(2)
            .and_then(|s| s.as_bytes().first())
            .copied()
            .ok_or("unsupported")?;
        let mut bytes = vec![];
        std::io::stdin()
            .take(INPUT_LIMIT as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "read_failed")?;
        extract(kind, &bytes)
    })
    .unwrap_or_else(|_| Err("extraction_failed".into()));
    // Error strings may originate in a text decoder: expose a fixed status only.
    let reply = result.map_err(|_| "extraction_failed".to_owned());
    if let Ok(bytes) = serde_json::to_vec(&reply) {
        let _ = std::io::stdout().write_all(&bytes);
    }
    true
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub fn pdf() -> Vec<u8> {
        use pdf_extract::{dictionary, Object, Stream};
        let mut doc = pdf_extract::Document::with_version("1.5");
        let pages = doc.new_object_id();
        let font =
            doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
        let content = doc.add_object(Stream::new(
            dictionary! {},
            b"BT /F1 12 Tf 72 700 Td (quartzpdfneedle generated text) Tj ET".to_vec(),
        ));
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"Contents"=>content,"MediaBox"=>vec![0.into(),0.into(),612.into(),792.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}});
        doc.objects.insert(
            pages,
            dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into(),
        );
        let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
        doc.trailer.set("Root", root);
        let mut bytes = vec![];
        doc.save_to(&mut bytes).unwrap();
        bytes
    }
    pub fn docx(xml: &str) -> Vec<u8> {
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        archive
            .start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(xml.as_bytes()).unwrap();
        archive.finish().unwrap().into_inner()
    }
    #[test]
    fn real_pdf_embedded_text_is_decoded_and_bad_documents_refused() {
        assert!(extract(b'p', &pdf())
            .unwrap()
            .text
            .contains("quartzpdfneedle"));
        assert!(extract(b'p', b"not a pdf").is_err());
        assert!(extract(b'p', b"%PDF-1.7\ninvalid").is_err());
        assert_eq!(kind("doc", "old.doc"), None);
    }
    #[test]
    fn docx_runs_entities_paragraphs_and_tabs_are_text_not_executable_markup() {
        let bytes = docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>amethystdocxneedle &amp; اردو</w:t><w:tab/><w:t>two</w:t></w:r></w:p><w:p><w:r><w:t>&#65;</w:t></w:r></w:p></w:body></w:document>"#,
        );
        let result = extract(b'd', &bytes).unwrap();
        assert!(result.text.contains("amethystdocxneedle & اردو\ttwo\nA"));
        assert!(!result.truncated);
        assert!(extract(b'd', b"bad zip").is_err());
        assert!(extract(
            b'd',
            &docx("<!DOCTYPE x [<!ENTITY x SYSTEM 'file:///secret'>]><x><t>&x;</t></x>")
        )
        .is_err());
        assert!(extract(b'd', &docx(&"x".repeat(4 * 1024 * 1024 + 1))).is_err());
    }
    #[test]
    fn unicode_encodings_binary_refusal_and_bounded_partial_text() {
        assert_eq!(
            extract(b't', "اردو sapphiretextneedle".as_bytes())
                .unwrap()
                .text,
            "اردو sapphiretextneedle"
        );
        let utf16 = [0xff, 0xfe]
            .into_iter()
            .chain("hello unicode".encode_utf16().flat_map(u16::to_le_bytes))
            .collect::<Vec<_>>();
        assert_eq!(extract(b't', &utf16).unwrap().text, "hello unicode");
        assert!(extract(b't', b"binary\0data").is_err());
        assert!(extract(b't', &[0xff, 0xff, 0xff]).is_err());
        let long = "ی".repeat(OUTPUT_LIMIT);
        let result = extract(b't', long.as_bytes()).unwrap();
        assert!(result.truncated && result.text.len() <= OUTPUT_LIMIT);
        assert!(result.text.is_char_boundary(result.text.len()));
        assert!(extract(b'p', &vec![0u8; INPUT_LIMIT + 1]).is_err());
    }
}
