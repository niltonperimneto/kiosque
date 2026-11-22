#!/usr/bin/env python3
"""Download only locked upstream assets; verify before making them executable."""
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import urllib.request
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]


class AssetRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, new_url):
        if urlsplit(new_url).scheme != 'https':
            raise RuntimeError('Insecure build-tool redirect')
        redirected = super().redirect_request(request, response, code, message, headers, new_url)
        if redirected is not None and urlsplit(new_url).netloc != urlsplit(request.full_url).netloc:
            redirected.remove_header('Authorization')
        return redirected


def download(name, destination):
    entry = json.loads((ROOT / '.github/build-tools.lock.json').read_text())[name]
    url = f"https://api.github.com/repos/{entry['repository']}/releases/assets/{entry['asset_id']}"
    request = urllib.request.Request(url, headers={'Accept': 'application/octet-stream', 'User-Agent': 'Kiosque-build'})
    if token := os.environ.get('GITHUB_TOKEN'):
        request.add_header('Authorization', f'Bearer {token}')
    opener = urllib.request.build_opener(AssetRedirectHandler())
    destination = Path(destination)
    digest = hashlib.sha256()
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as output:
            temporary = Path(output.name)
            with opener.open(request, timeout=60) as response:
                if not response.url.startswith('https://'):
                    raise RuntimeError('Insecure build-tool redirect')
                size = 0
                while block := response.read(1024 * 1024):
                    size += len(block)
                    if size > 256 * 1024 * 1024:
                        raise RuntimeError('Build tool exceeds download limit')
                    digest.update(block)
                    output.write(block)
        if digest.hexdigest() != entry['sha256']:
            raise RuntimeError(f'Checksum mismatch for {name}; refusing to execute')
        temporary.chmod(0o755)
        temporary.replace(destination)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


if __name__ == '__main__':
    download(*sys.argv[1:])
