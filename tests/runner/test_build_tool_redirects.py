import importlib.util
from pathlib import Path
import unittest
import urllib.request

spec = importlib.util.spec_from_file_location('download', Path(__file__).resolve().parents[2] / 'scripts/download_build_tool.py')
download = importlib.util.module_from_spec(spec)
spec.loader.exec_module(download)


class BuildToolRedirectTests(unittest.TestCase):
    def test_asset_redirect_removes_api_credentials(self):
        request = urllib.request.Request('https://api.github.com/repos/example/releases/assets/1',
                                         headers={'Authorization': 'Bearer synthetic'})
        redirected = download.AssetRedirectHandler().redirect_request(
            request, None, 302, 'Found', {}, 'https://release-assets.githubusercontent.com/asset')
        self.assertIsNone(redirected.get_header('Authorization'))

    def test_insecure_redirect_is_rejected(self):
        request = urllib.request.Request('https://api.github.com/asset')
        with self.assertRaisesRegex(RuntimeError, 'Insecure'):
            download.AssetRedirectHandler().redirect_request(request, None, 302, 'Found', {}, 'http://example.org/asset')
