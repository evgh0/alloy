# Alloy

This project is a small Rust + WebAssembly WebGL2 demo.

## Build

Generate the browser package with:

```powershell
wasm-pack build --target web
```

## Serve

Run the local Python server from the project root:

```powershell
python serve.py
```

Then open:

```text
http://127.0.0.1:8000/
```

The server sets explicit MIME types for `.js` and `.wasm` files so the browser can load `pkg/alloy.js` without a blocked MIME-type error.

## Camera controls

- `Ctrl + Left Mouse Drag`: orbit
- `Ctrl + Middle Mouse Drag`: pan
- `Ctrl + Right Mouse Drag` or mouse wheel: dolly/zoom
- `F`: toggle freeflight mode
- `W / A / S / D`: move in freeflight
- `Space`: move up in freeflight
- `Shift`: move down in freeflight

Note: `Ctrl + wheel` can still clash with browser zoom behavior on some browsers/platforms, but the canvas listeners prevent the default wheel/context-menu actions where possible.

