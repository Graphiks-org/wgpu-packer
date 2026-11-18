#!/usr/bin/env python3
"""One-shot transformer (already applied — kept for history;
new entry points should call contain_entry directly): wraps every #[no_mangle] extern "C" fn body of the
FFI crate in ffi_utils::contain_entry, so no panic crosses the C ABI.

String/comment-aware brace matching; bodies are re-indented by 4 spaces
(the files contain no raw strings, verified).
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

FALLBACK = {
    "()": "()",
    "native::WGPUStatus": "native::WGPUStatus_Error",
    "native::WGPUWaitStatus": "native::WGPUWaitStatus_Error",
    "native::WGPUFuture": "native::WGPUFuture { id: 0 }",
    "native::WGPUProc": "None",
    "bool": "false",
    "f32": "0.0",
    "u32": "0",
    "u64": "0",
    "usize": "0",
    "std::os::raw::c_uint": "0",
    # enum aliases (all c_uint underneath): 0 = Undefined / Unmapped / none
    "native::WGPUBool": "0",
    "native::WGPUTextureFormat": "0",
    "native::WGPUTextureUsage": "0",
    "native::WGPUBufferUsage": "0",
    "native::WGPUTextureViewDimension": "0",
    "native::WGPUTextureDimension": "0",
    "native::WGPUBufferMapState": "0",
    "native::WGPUQueryType": "0",
    "native::WGPUSubmissionIndex": "0",
    # raw pointers (handle typedefs resolve to *const T)
    "*mut c_void": "std::ptr::null_mut()",
    "*mut std::ffi::c_void": "std::ptr::null_mut()",
    "*mut u8": "std::ptr::null_mut()",
    "*const u8": "std::ptr::null()",
}

HANDLE_RE = re.compile(r"^native::WGPU[A-Za-z]+$")

def fallback_for(ret: str) -> str:
    ret = ret.strip()
    if ret in FALLBACK:
        return FALLBACK[ret]
    if HANDLE_RE.match(ret):
        # handle typedefs are *const ...Impl: NULL is the safe answer
        return "std::ptr::null()"
    raise SystemExit(f"no fallback rule for return type: {ret!r}")

def scan_body(text: str, open_idx: int) -> int:
    """Index of the brace matching the one at open_idx, skipping strings,
    chars and comments."""
    i = open_idx
    depth = 0
    n = len(text)
    while i < n:
        c = text[i]
        if c == '"':
            i += 1
            while i < n and text[i] != '"':
                if text[i] == '\\':
                    i += 1
                i += 1
        elif c == "'":
            # char literal vs lifetime: '\...' or '<c>' pattern
            if i + 1 < n and text[i + 1] == '\\':
                i += 2
                while i < n and text[i] != "'":
                    i += 1
            elif i + 2 < n and text[i + 2] == "'" and text[i + 1] != "'":
                i += 2
        elif c == '/' and i + 1 < n and text[i + 1] == '/':
            while i < n and text[i] != '\n':
                i += 1
        elif c == '/' and i + 1 < n and text[i + 1] == '*':
            i += 2
            while i + 1 < n and not (text[i] == '*' and text[i + 1] == '/'):
                i += 1
            i += 1
        elif c == '{':
            depth += 1
        elif c == '}':
            depth -= 1
            if depth == 0:
                return i
        i += 1
    raise SystemExit("unbalanced braces")

def transform(path: pathlib.Path) -> tuple[int, int]:
    text = path.read_text()
    marker = "\n#[no_mangle]\n"
    parts = text.split(marker)
    if len(parts) == 1:
        return 0, 0
    wrapped = 0
    skipped = 0
    out = [parts[0]]
    for seg in parts[1:]:
        m = re.search(r'pub (?:unsafe )?extern "C" fn (\w+)\(', seg)
        if not m or "unimplemented!()" in seg and False:
            out.append(seg)
            continue
        name = m.group(1)
        # signature closing paren
        depth = 0
        i = m.end() - 1  # at '('
        n = len(seg)
        while i < n:
            if seg[i] == '(':
                depth += 1
            elif seg[i] == ')':
                depth -= 1
                if depth == 0:
                    break
            i += 1
        # return type until the body's opening brace
        rest = seg[i + 1:]
        arrow = re.match(r'\s*(->\s*(.+?))?\s*\{', rest, re.S)
        if not arrow:
            out.append(seg)
            skipped += 1
            continue
        ret = arrow.group(2) if arrow.group(2) else "()"
        body_open = i + 1 + arrow.end() - 1  # index of '{' in seg
        body_close = scan_body(seg, body_open)
        body = seg[body_open + 1:body_close]
        prefix = seg[:body_open + 1]
        suffix = seg[body_close:]  # '}' + trailing comments
        fb = fallback_for(ret)
        indented = "".join(
            ("    " + line if line.strip() else line) + "\n"
            for line in body.rstrip("\n").split("\n")
        )
        new = (
            prefix
            + "\n    $crate::ffi_utils::contain_entry(\n"
            + f'        "{name}",\n'
            + f"        {fb},\n"
            + "        || {\n"
            + indented
            + "        },\n"
            + "    )\n"
            + suffix
        )
        out.append(new)
        wrapped += 1
    path.write_text(marker.join(out))
    return wrapped, skipped

total_w = total_s = 0
for name in [
    "adapter", "bind_group", "buffer", "command", "compute", "device",
    "instance", "labels", "pipeline", "query", "queue", "render",
    "sampler", "shader", "surface", "texture",
]:
    w, s = transform(ROOT / "ffi/src/api" / f"{name}.rs")
    if w or s:
        print(f"{name}.rs: wrapped={w} skipped={s}")
    total_w += w
    total_s += s
for extra in ["ffi/src/logging.rs", "ffi/src/unimplemented.rs"]:
    w, s = transform(ROOT / extra)
    print(f"{extra}: wrapped={w} skipped={s}")
    total_w += w
    total_s += s
print(f"TOTAL wrapped={total_w} skipped={total_s}")
