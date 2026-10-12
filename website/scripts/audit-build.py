"""Dependency-free production-artifact checks; no network or server."""
from html.parser import HTMLParser
from pathlib import Path
import gzip
import json
import re
import struct
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
DIST = ROOT / 'dist'
BASE = '/configlens/'
CANONICAL = 'https://ob4cl.github.io/configlens/'


class Audit(HTMLParser):
    def __init__(self):
        super().__init__()
        self.ids = []
        self.headings = []
        self.links = []
        self.assets = []
        self.meta = {}
        self.canonical = None
        self.json_ld = ''
        self.in_json_ld = False

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        assert not any(name.startswith('on') for name in a), 'inline event handler'
        if 'id' in a:
            self.ids.append(a['id'])
        if tag == 'h1':
            self.headings.append(tag)
        if tag == 'a':
            self.links.append(a.get('href', ''))
        if tag == 'link' and a.get('rel') == 'canonical':
            self.canonical = a['href']
        if tag == 'meta':
            self.meta[a.get('name') or a.get('property')] = a.get('content')
        if tag == 'script':
            if 'src' in a:
                self.assets.append(a['src'])
            else:
                assert a.get('type') == 'application/ld+json', 'unexpected inline JS'
                self.in_json_ld = True
        if tag == 'img':
            assert 'alt' in a
            self.assets.append(a['src'])
        if tag == 'link' and a.get('rel') in ('stylesheet', 'icon'):
            self.assets.append(a['href'])

    def handle_endtag(self, tag):
        if tag == 'script':
            self.in_json_ld = False

    def handle_data(self, data):
        if self.in_json_ld:
            self.json_ld += data


audit = Audit()
audit.feed((DIST / 'index.html').read_text())
assert len(audit.headings) == 1
assert len(audit.ids) == len(set(audit.ids))
assert audit.canonical == CANONICAL
for key in ('description', 'og:title', 'og:description', 'og:url', 'og:image', 'twitter:card', 'twitter:image'):
    assert audit.meta.get(key), key
assert audit.meta['og:url'] == CANONICAL
assert audit.meta['og:image'] == CANONICAL + 'og.png'
structured = json.loads(audit.json_ld)
assert structured['@type'] == 'SoftwareSourceCode'
assert structured['codeRepository'] == 'https://github.com/ob4cl/configlens'
assert structured['version'] == '0.1.0-beta.3'
for asset in audit.assets:
    assert asset.startswith(BASE), f'remote/mis-based asset: {asset}'
    assert (DIST / asset.removeprefix(BASE)).is_file(), asset
for link in audit.links:
    assert link
    if link.startswith('#'):
        assert link == '#' or link[1:] in audit.ids, link
    else:
        assert link.startswith('https://github.com/ob4cl/configlens'), link
        if '/blob/main/' in link:
            path = link.split('/blob/main/', 1)[1].split('#')[0]
            assert (ROOT.parent / path).is_file(), path
png = (DIST / 'og.png').read_bytes()
assert png[:8] == b'\x89PNG\r\n\x1a\n'
assert struct.unpack('>II', png[16:24]) == (1200, 630)
sitemap = ET.parse(DIST / 'sitemap.xml')
location = sitemap.find('.//{http://www.sitemaps.org/schemas/sitemap/0.9}loc')
assert location is not None and location.text == CANONICAL
assets = []
for file in sorted(DIST.rglob('*')):
    if file.is_file():
        data = file.read_bytes()
        assets.append({'path': str(file.relative_to(DIST)), 'bytes': len(data), 'gzipBytes': len(gzip.compress(data))})
        if file.suffix in ('.js', '.css'):
            text = data.decode()
            assert not re.search(r'\b(?:fetch|WebSocket|EventSource)\s*\(', text)
            assert 'innerHTML' not in text and 'document.write' not in text
            if file.suffix == '.css':
                assert not re.search(r'@import|url\(\s*["\']?https?://', text)
result = {'singleH1': True, 'uniqueIds': True, 'localAssets': True, 'validAnchorsAndRepoFiles': True, 'metadataAndSchema': True, 'og1200x630': True, 'noRuntimeNetworkOrHtmlSinks': True, 'assets': assets}
print(json.dumps(result, indent=2))
