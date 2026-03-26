from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


class WasmRequestHandler(SimpleHTTPRequestHandler):
    extensions_map = SimpleHTTPRequestHandler.extensions_map.copy()
    extensions_map.update(
        {
            ".js": "application/javascript",
            ".mjs": "application/javascript",
            ".wasm": "application/wasm",
        }
    )


def main() -> None:
    root = Path(__file__).resolve().parent
    handler = partial(WasmRequestHandler, directory=str(root))
    server = ThreadingHTTPServer(("127.0.0.1", 8000), handler)

    print(f"Serving {root} on http://127.0.0.1:8000/")
    print("Open that URL in the browser after running: wasm-pack build --target web")
    server.serve_forever()


if __name__ == "__main__":
    main()