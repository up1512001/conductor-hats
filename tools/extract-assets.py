#!/usr/bin/env python3
"""Extract Conductor's embedded frontend out of its Mach-O binary.

    tools/extract-assets.py list
    tools/extract-assets.py dump <outdir>
    tools/extract-assets.py grep <pattern>

Tauri compiles the frontend into the executable rather than shipping files, so
there is no .js or .html anywhere in the app bundle. It is still all there: the
asset map lives in __DATA_CONST as 32-byte entries of

    (key_ptr: u64, key_len: u64, value_ptr: u64, value_len: u64)

with plaintext keys like "/assets/renderApp-Do42UGbm.css" and brotli-compressed
values. This walks that map.

Read-only. Nothing here writes to the app.
"""

import argparse
import pathlib
import re
import struct
import subprocess
import sys

DEFAULT_BINARY = "/Applications/Conductor.app/Contents/MacOS/conductor"


class MachO:
    def __init__(self, path):
        self.path = pathlib.Path(path)
        self.data = self.path.read_bytes()
        self.segments = self._segments()

    def _segments(self):
        d = self.data
        if struct.unpack_from("<I", d, 0)[0] not in (0xFEEDFACF, 0xCFFAEDFE):
            sys.exit(f"{self.path}: not a 64-bit Mach-O (fat binaries are not handled)")
        ncmds = struct.unpack_from("<I", d, 16)[0]
        off, segs = 32, []
        for _ in range(ncmds):
            cmd, cmdsize = struct.unpack_from("<2I", d, off)
            if cmd == 0x19:  # LC_SEGMENT_64
                name = d[off + 8 : off + 24].rstrip(b"\0").decode()
                vmaddr, vmsize, fileoff, filesize = struct.unpack_from("<4Q", d, off + 24)
                segs.append((name, vmaddr, vmsize, fileoff, filesize))
            off += cmdsize
        return segs

    def to_file(self, vmaddr):
        """Virtual address to file offset, or None when it is not backed by the file."""
        for _, vm, vmsize, fileoff, filesize in self.segments:
            if vm <= vmaddr < vm + vmsize and filesize:
                delta = vmaddr - vm
                if delta < filesize:
                    return fileoff + delta
        return None

    def to_vm(self, fileoff):
        for _, vm, _, f0, filesize in self.segments:
            if f0 <= fileoff < f0 + filesize:
                return vm + (fileoff - f0)
        return None

    def segment(self, name):
        for seg in self.segments:
            if seg[0] == name:
                return seg
        return None


def find_assets(macho):
    """Every (key, value_fileoff, value_len) triple in the embedded asset map.

    Rather than guess where the map starts, this finds the asset keys by their
    text and then looks for the pointer to each one, which is what an entry is.
    """
    d = macho.data
    seg = macho.segment("__DATA_CONST")
    if not seg:
        sys.exit("no __DATA_CONST segment")
    _, _, _, const_off, const_size = seg
    const = d[const_off : const_off + const_size]

    keys = {}
    for m in re.finditer(rb"/(?:assets/|)[A-Za-z0-9._/-]{3,80}\.(?:js|css|html|svg|png|woff2?|json|map)", d):
        vm = macho.to_vm(m.start())
        if vm is not None:
            keys.setdefault(vm, m.group())

    found = []
    for vm, key in keys.items():
        needle = struct.pack("<Q", vm)
        pos = const.find(needle)
        if pos < 0:
            continue
        try:
            k_ptr, k_len, v_ptr, v_len = struct.unpack_from("<4Q", const, pos)
        except struct.error:
            continue
        if k_ptr != vm or k_len != len(key):
            continue
        if not (0 < v_len < 64 * 1024 * 1024):
            continue
        v_off = macho.to_file(v_ptr)
        if v_off is None or v_off + v_len > len(d):
            continue
        found.append((key.decode(), v_off, v_len, const_off + pos))
    found.sort(key=lambda e: e[0])
    return found


def brotli_decompress(blob):
    """Decompress via node, which has brotli in its standard library.

    Avoids depending on a pip package that macOS does not ship.
    """
    try:
        import brotli  # noqa: F401  (used when available)

        return brotli.decompress(blob)
    except ImportError:
        pass
    proc = subprocess.run(
        ["node", "-e",
         "const c=[];process.stdin.on('data',d=>c.push(d)).on('end',()=>"
         "process.stdout.write(require('zlib').brotliDecompressSync(Buffer.concat(c))))"],
        input=blob, capture_output=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.decode()[:200] or "brotli failed")
    return proc.stdout


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["list", "dump", "grep"])
    ap.add_argument("arg", nargs="?")
    ap.add_argument("--binary", default=DEFAULT_BINARY)
    args = ap.parse_args()

    macho = MachO(args.binary)
    assets = find_assets(macho)
    if not assets:
        sys.exit("found no assets; the layout may have changed in this version")

    if args.command == "list":
        total = 0
        for key, off, ln, entry in assets:
            print(f"{ln:9}  {off:9}  entry@{entry:9}  {key}")
            total += ln
        print(f"\n{len(assets)} assets, {total / 1024:.0f} KiB compressed")
        return

    if args.command == "dump":
        outdir = pathlib.Path(args.arg or "conductor-assets")
        outdir.mkdir(parents=True, exist_ok=True)
        ok = bad = 0
        for key, off, ln, _ in assets:
            blob = macho.data[off : off + ln]
            try:
                content = brotli_decompress(blob)
            except Exception as e:
                print(f"  skip {key}: {e}", file=sys.stderr)
                bad += 1
                continue
            dest = outdir / key.lstrip("/")
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(content)
            ok += 1
        print(f"{ok} written to {outdir}" + (f", {bad} failed" if bad else ""))
        return

    if args.command == "grep":
        if not args.arg:
            sys.exit("grep needs a pattern")
        pat = re.compile(args.arg.encode(), re.I)
        for key, off, ln, _ in assets:
            try:
                content = brotli_decompress(macho.data[off : off + ln])
            except Exception:
                continue
            hits = list(pat.finditer(content))
            if hits:
                print(f"\n=== {key}  ({len(hits)} hits, {len(content)} bytes) ===")
                for m in hits[:3]:
                    s = max(0, m.start() - 90)
                    print("   ", content[s : m.end() + 90].decode("utf-8", "replace").replace("\n", " "))


if __name__ == "__main__":
    main()
