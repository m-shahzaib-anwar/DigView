"""Recover slow Cargo downloads via the official registry using normal TLS.

Development-only helper. Cargo still resolves versions itself, and every crate
archive is checked against Cargo.lock's SHA-256. No registry mirror or TLS bypass.
"""
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
import argparse
import hashlib
import json
import ipaddress
import os
import re
import subprocess
import tomllib

parser = argparse.ArgumentParser()
parser.add_argument('--manifest', default='native-core/Cargo.toml')
parser.add_argument('--static-ip', help='Optional current DNS-resolved address for the official static.crates.io CDN')
parser.add_argument('--index-ip', help='Optional current DNS-resolved address for the official index.crates.io CDN')
options = parser.parse_args()
if options.static_ip:
    ipaddress.ip_address(options.static_ip)
if options.index_ip:
    ipaddress.ip_address(options.index_ip)
root = Path(__file__).resolve().parent.parent
manifest = (root / options.manifest).resolve()
if not manifest.is_relative_to(root):
    raise SystemExit('Manifest must be inside this project')
cargo_home = root / '.tools/cargo'
toolchain = root / '.tools/toolchain/bin'
env = dict(os.environ, CARGO_HOME=str(cargo_home), PATH=str(toolchain) + os.pathsep + os.environ['PATH'])
cargo = str(toolchain / 'cargo.exe')
registry = cargo_home / 'registry/index/index.crates.io-1949cf8c6b5b557f'
registry.mkdir(parents=True, exist_ok=True)
scratch = root / '.tools/registry-downloads'
scratch.mkdir(parents=True, exist_ok=True)
fetched = set()

def index_path(name):
    if not re.fullmatch(r'[a-zA-Z0-9_-]+', name):
        raise RuntimeError('Invalid crate name')
    name = name.lower()
    return (f'1/{name}' if len(name) == 1 else f'2/{name}' if len(name) == 2 else
            f'3/{name[0]}/{name}' if len(name) == 3 else f'{name[:2]}/{name[2:4]}/{name}')

def fetch_index(name):
    relative = index_path(name)
    target = scratch / f'{name}.json'
    connection = ['--resolve', f'index.crates.io:443:{options.index_ip}'] if options.index_ip else []
    subprocess.run(['curl.exe', '--silent', '--show-error', '--fail', '--location', *connection,
                    '--connect-timeout', '15', '--max-time', '60', '--retry', '2',
                    '--output', str(target), f'https://index.crates.io/{relative}'], check=True)
    rows = [json.loads(line) for line in target.read_bytes().splitlines() if line]
    if not rows or any(row['name'].lower() != name.lower() for row in rows):
        raise RuntimeError(f'Invalid index response for {name}')
    # Cargo sparse-index cache v3, as observed in the freshly created local cache.
    data = bytearray(b'\x03\x02\x00\x00\x00etag: "direct-official-fetch"\x00')
    for row in rows:
        data += row['vers'].encode() + b'\x00' + json.dumps(row, separators=(',', ':')).encode() + b'\x00'
    cached = registry / '.cache' / relative
    cached.parent.mkdir(parents=True, exist_ok=True)
    cached.write_bytes(data)
    print(f'Indexed {name}', flush=True)

def cached_rows(name):
    cached = registry / '.cache' / index_path(name)
    return [json.loads(part) for part in cached.read_bytes().split(b'\x00') if part.startswith(b'{')]

def expand_candidates(name, versions):
    """Fetch candidate dependency metadata; Cargo remains the version resolver."""
    pending = [(name, set(versions), 0, ('default',))]
    visited = set()
    count = 0
    while pending:
        package, candidates, depth, enabled_features = pending.pop()
        identity = (package, tuple(sorted(candidates)), enabled_features)
        if identity in visited or depth > 4:
            continue
        visited.add(identity)
        for row in cached_rows(package):
            if row['vers'] not in candidates:
                continue
            features = {**row.get('features', {}), **row.get('features2', {})}
            feature_queue = list(enabled_features)
            seen_features, optional_dependencies = set(), set()
            while feature_queue:
                feature = feature_queue.pop()
                if feature in seen_features:
                    continue
                seen_features.add(feature)
                if feature.startswith('dep:'):
                    optional_dependencies.add(feature[4:])
                elif '/' in feature:
                    dependency_feature = feature.split('/')[0]
                    if not dependency_feature.endswith('?'):
                        optional_dependencies.add(dependency_feature)
                elif feature in features:
                    feature_queue.extend(features[feature])
                else:
                    optional_dependencies.add(feature)
            for dependency in row['deps']:
                if dependency.get('kind') == 'dev':
                    continue
                if dependency.get('optional') and dependency['name'] not in optional_dependencies:
                    continue
                dependency_name = dependency.get('package') or dependency['name']
                if not (registry / '.cache' / index_path(dependency_name)).exists():
                    fetch_index(dependency_name)
                    count += 1
                requirement = re.search(r'(\d+)\.(\d+)', dependency['req'])
                if not requirement:
                    continue
                major, minor = requirement.groups()
                family = major + '.' if major != '0' else f'0.{minor}.'
                versions = {candidate['vers'] for candidate in cached_rows(dependency_name)
                            if candidate['vers'].startswith(family) and '-' not in candidate['vers']}
                versions = set(sorted(versions, key=lambda value: tuple(int(part) for part in value.split('+')[0].split('.')), reverse=True)[:3])
                requested = tuple(dependency.get('features', []) + (['default'] if dependency.get('default_features', True) else []))
                pending.append((dependency_name, versions, depth + 1, requested))
    return count

for _ in range(900):
    completed = subprocess.run([cargo, 'generate-lockfile', '--offline', '--manifest-path', str(manifest)],
                               env=env, capture_output=True, text=True, timeout=60)
    if completed.returncode == 0:
        print(completed.stderr.strip(), flush=True)
        break
    match = re.search(r'no matching package named `([^`]+)` found', completed.stderr)
    if not match:
        match = re.search(r'failed to select a version for the requirement `([^ =`]+)', completed.stderr)
    if not match:
        # Offline resolution can report a version conflict when a candidate's
        # dependency index is absent. Expand only the candidates Cargo named.
        conflict = re.search(r'failed to select a version for `([^`]+)`', completed.stderr)
        versions = re.search(r'versions that meet the requirements[^\n]+are: ([^\n]+)', completed.stderr)
        if conflict and versions:
            name = conflict[1]
            candidates = set(versions[1].split(', '))
            if expand_candidates(name, candidates):
                continue
    if not match or match[1] in fetched:
        raise SystemExit(completed.stderr)
    name = match[1]
    fetched.add(name)
    fetch_index(name)
else:
    raise SystemExit('Registry recovery exceeded its package limit')

lock = tomllib.loads(manifest.with_name('Cargo.lock').read_text())
packages = [p for p in lock['package'] if p.get('source', '').startswith('registry+')]
cache = cargo_home / 'registry/cache/index.crates.io-1949cf8c6b5b557f'
cache.mkdir(parents=True, exist_ok=True)

def digest(path):
    with path.open('rb') as file:
        return hashlib.file_digest(file, 'sha256').hexdigest()

def download(package):
    name, version, checksum = package['name'], package['version'], package['checksum']
    if not re.fullmatch(r'[a-zA-Z0-9_-]+', name) or not re.fullmatch(r'[a-zA-Z0-9.+_-]+', version):
        raise RuntimeError('Invalid locked package identity')
    target = cache / f'{name}-{version}.crate'
    if target.exists() and digest(target) == checksum:
        return
    partial = target.with_suffix('.part')
    for attempt in range(3):
        connection = ['--resolve', f'static.crates.io:443:{options.static_ip}'] if options.static_ip else []
        result = subprocess.run(['curl.exe', '--silent', '--show-error', '--fail', '--location', *connection,
                                 '--connect-timeout', '15', '--max-time', '180', '--continue-at', '-',
                                 '--output', str(partial), f'https://static.crates.io/crates/{name}/{name}-{version}.crate'])
        if result.returncode == 0 and digest(partial) == checksum:
            partial.replace(target)
            print(f'Verified {name} {version}', flush=True)
            return
        if result.returncode == 0:
            partial.unlink(missing_ok=True)
    raise RuntimeError(f'Download incomplete or checksum mismatch: {name} {version}')

with ThreadPoolExecutor(max_workers=6) as executor:
    jobs = [executor.submit(download, package) for package in packages]
    for job in as_completed(jobs):
        job.result()
print(f'Official Cargo dependencies ready: {len(packages)} archives. Run Cargo with --offline.', flush=True)
