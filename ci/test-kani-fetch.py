#!/usr/bin/env python3
"""Check original-download bounds, transport refusal and output preservation."""
from pathlib import Path
import email.message
import hashlib
import importlib.util
import io
import tempfile
import urllib.response
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("sail_kani_fetch", Path(__file__).with_name("kani-fetch.py"))
fetcher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fetcher)


class Response(io.BytesIO):
    def __init__(self, value, url="https://example.invalid/original"):
        super().__init__(value)
        self.url = url

    def geturl(self):
        return self.url


class DownloadTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="sail-kani-fetch-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.output = self.root / "original"
        self.url = "https://example.invalid/original"
        self.sha = hashlib.sha256(b"abc").hexdigest()

    def test_exact_original_bytes(self):
        with mock.patch.object(fetcher.urllib.request, "build_opener", return_value=mock.Mock(open=mock.Mock(return_value=Response(b"abc")))):
            fetcher.fetch(self.url, self.output, 3, self.sha)
        self.assertEqual(self.output.read_bytes(), b"abc")

    def test_truncated_excessive_or_modified_stream_refuses(self):
        for data in [b"ab", b"abcd", b"abd"]:
            with self.subTest(data=data):
                with mock.patch.object(fetcher.urllib.request, "build_opener", return_value=mock.Mock(open=mock.Mock(return_value=Response(data)))):
                    with self.assertRaises(ValueError): fetcher.fetch(self.url, self.output, 3, self.sha)
                if self.output.exists(): self.output.unlink()

    def test_non_https_request_and_redirect_refuse(self):
        with mock.patch.object(fetcher.urllib.request, "build_opener") as transport:
            with self.assertRaises(ValueError): fetcher.fetch("http://example.invalid/original", self.output, 3, self.sha)
            transport.assert_not_called()
        with mock.patch.object(fetcher.urllib.request, "build_opener", return_value=mock.Mock(open=mock.Mock(return_value=Response(b"abc", "http://example.invalid/original")))):
            with self.assertRaises(ValueError): fetcher.fetch(self.url, self.output, 3, self.sha)
        self.assertFalse(self.output.exists())

    def test_redirect_chain_refuses_before_any_http_contact(self):
        original_build = fetcher.urllib.request.build_opener
        for target in ["http://downgrade.invalid/transit", "ftp://downgrade.invalid/transit"]:
            with self.subTest(target=target):
                requests = []

                class Transport(fetcher.urllib.request.BaseHandler):
                    handler_order = 100

                    def https_open(self, request):
                        requests.append(request.full_url)
                        headers = email.message.Message()
                        headers["Location"] = target
                        response = urllib.response.addinfourl(io.BytesIO(b""), headers, request.full_url, 302)
                        response.msg = "synthetic redirect"
                        return response

                    def http_open(self, request):
                        raise AssertionError("downgrade transport contacted")

                    ftp_open = http_open

                def opener(*handlers):
                    return original_build(fetcher.urllib.request.ProxyHandler({}), Transport(), *handlers)

                with mock.patch.object(fetcher.urllib.request, "build_opener", side_effect=opener):
                    with self.assertRaises(ValueError):
                        fetcher.fetch(self.url, self.output, 3, self.sha)
                self.assertEqual(requests, [self.url])
                self.assertFalse(self.output.exists())

    def test_https_redirect_keeps_exact_byte_authentication(self):
        original_build = fetcher.urllib.request.build_opener
        requests = []

        class Transport(fetcher.urllib.request.BaseHandler):
            handler_order = 100

            def https_open(self, request):
                requests.append(request.full_url)
                headers = email.message.Message()
                first = len(requests) == 1
                if first:
                    headers["Location"] = "https://example.invalid/final"
                response = urllib.response.addinfourl(io.BytesIO(b"" if first else b"abc"), headers, request.full_url, 302 if first else 200)
                response.msg = "synthetic response"
                return response

        def opener(*handlers):
            return original_build(fetcher.urllib.request.ProxyHandler({}), Transport(), *handlers)

        with mock.patch.object(fetcher.urllib.request, "build_opener", side_effect=opener):
            fetcher.fetch(self.url, self.output, 3, self.sha)
        self.assertEqual(len(requests), 2)
        self.assertEqual(self.output.read_bytes(), b"abc")

    def test_invalid_bounds_and_digests_refuse_without_network(self):
        for size,digest in [(0,self.sha),(-1,self.sha),(True,self.sha),(fetcher.preparation.rust.MAX_ARCHIVE+1,self.sha),(3,"0"),(3,"G"*64)]:
            with self.subTest(size=size,digest=digest), mock.patch.object(fetcher.urllib.request, "build_opener") as transport:
                with self.assertRaises(ValueError): fetcher.fetch(self.url,self.output,size,digest)
                transport.assert_not_called()

    def test_existing_file_and_dangling_link_are_preserved(self):
        for linked in [False,True]:
            with self.subTest(linked=linked):
                if linked:self.output.symlink_to(self.root/'absent')
                else:self.output.write_bytes(b"keep")
                with mock.patch.object(fetcher.urllib.request, "build_opener", return_value=mock.Mock(open=mock.Mock(return_value=Response(b"abc")))):
                    with self.assertRaises(FileExistsError):fetcher.fetch(self.url,self.output,3,self.sha)
                if linked:self.assertTrue(self.output.is_symlink())
                else:self.assertEqual(self.output.read_bytes(),b"keep")
                self.output.unlink()

    def test_existing_runtime_refuses_before_reading_pins_or_network(self):
        self.output.mkdir()
        with mock.patch.object(fetcher.urllib.request, "build_opener") as transport:
            with self.assertRaises(ValueError):fetcher.download(self.root/'missing-pin',self.root/'missing-pin',self.output)
            transport.assert_not_called()
        self.assertEqual(list(self.output.iterdir()),[])


if __name__ == "__main__":
    unittest.main()
