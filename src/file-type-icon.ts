import {
  Archive,
  File,
  FileArchive,
  FileAudio2,
  FileCode2,
  FileImage,
  FileJson2,
  FileSpreadsheet,
  FileText,
  FileType,
  FileType2,
  FileVideo2,
  Image,
  Music2,
  Package,
  Presentation,
  Video,
  type LucideIcon,
} from "lucide-react";
import type { Category } from "./types";

const extensionIcons: Record<string, LucideIcon> = {
  pdf: FileType2,
  txt: FileText,
  text: FileText,
  md: FileText,
  markdown: FileText,
  rtf: FileType,
  doc: FileType,
  docx: FileType,
  odt: FileType,
  xls: FileSpreadsheet,
  xlsx: FileSpreadsheet,
  xlsm: FileSpreadsheet,
  csv: FileSpreadsheet,
  tsv: FileSpreadsheet,
  ods: FileSpreadsheet,
  ppt: Presentation,
  pptx: Presentation,
  odp: Presentation,
  key: Presentation,
  json: FileJson2,
  jsonl: FileJson2,
  xml: FileCode2,
  html: FileCode2,
  htm: FileCode2,
  css: FileCode2,
  scss: FileCode2,
  js: FileCode2,
  jsx: FileCode2,
  ts: FileCode2,
  tsx: FileCode2,
  rs: FileCode2,
  py: FileCode2,
  java: FileCode2,
  c: FileCode2,
  h: FileCode2,
  cpp: FileCode2,
  cs: FileCode2,
  go: FileCode2,
  sql: FileCode2,
  sh: FileCode2,
  ps1: FileCode2,
  yaml: FileCode2,
  yml: FileCode2,
  toml: FileCode2,
  zip: FileArchive,
  rar: FileArchive,
  "7z": FileArchive,
  tar: FileArchive,
  gz: FileArchive,
  bz2: FileArchive,
  xz: FileArchive,
  zst: FileArchive,
  tgz: FileArchive,
  mp4: FileVideo2,
  mov: FileVideo2,
  mkv: FileVideo2,
  avi: FileVideo2,
  webm: FileVideo2,
  m4v: FileVideo2,
  mp3: FileAudio2,
  wav: FileAudio2,
  flac: FileAudio2,
  aac: FileAudio2,
  ogg: FileAudio2,
  m4a: FileAudio2,
  wma: FileAudio2,
  jpg: FileImage,
  jpeg: FileImage,
  png: FileImage,
  gif: FileImage,
  webp: FileImage,
  bmp: FileImage,
  tiff: FileImage,
  tif: FileImage,
  ico: FileImage,
  heic: FileImage,
  svg: FileImage,
  dmg: Package,
  msi: Package,
  exe: Package,
  app: Package,
  deb: Package,
  rpm: Package,
  apk: Package,
  aab: Package,
};

export function fileTypeIcon(
  extension: string,
  category: Category,
): LucideIcon {
  const normalized = extension.trim().replace(/^\./, "").toLowerCase();
  const icon = extensionIcons[normalized];
  if (icon) return icon;
  switch (category) {
    case "images":
      return Image;
    case "videos":
      return Video;
    case "music":
      return Music2;
    case "documents":
      return FileText;
    case "archives":
      return Archive;
    case "apps":
      return Package;
    default:
      return File;
  }
}
