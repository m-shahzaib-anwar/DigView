"""Recover a local toolchain from SHA-256-verified official Rust component archives.

Only used for this machine's interrupted download. Never extracts arbitrary paths
or modifies the system toolchain. Normal contributors should install Rustup.
"""
from pathlib import Path
import hashlib
import shutil
import tarfile

root = Path(__file__).resolve().parent.parent
destination = (root / '.tools/toolchain').resolve()
archives = {
    '4adb46c3707dee790682f7769d86983841148111695f561c843c33daa2af0c9b': 'cargo',
    '4fe09430ded40b8b3092b49f0a651178eb1f8154ef4d1f1a73f2a3595b2d956f': 'rust-std-x86_64-pc-windows-msvc',
    '7da83c2af1f209606b0160f25174d2867c7c7b73667c32c9e5fbcef4f5f45348': 'rustc',
}
for expected, component in archives.items():
    path = root / '.tools/rustup/downloads' / expected
    with path.open('rb') as file:
        actual = hashlib.file_digest(file, 'sha256').hexdigest()
    if actual != expected:
        raise RuntimeError(f'Checksum mismatch for {component}')
    with tarfile.open(path, 'r|xz') as archive:
        for member in archive:
            parts = Path(member.name).parts
            if not member.isfile() or len(parts) < 3 or parts[1] != component:
                continue
            target = destination.joinpath(*parts[2:]).resolve()
            if not target.is_relative_to(destination):
                raise RuntimeError('Archive path escaped toolchain directory')
            if target.name == 'manifest.in':
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.extractfile(member) as source, target.open('wb') as output:
                shutil.copyfileobj(source, output)
    print(f'Installed verified {component}', flush=True)
