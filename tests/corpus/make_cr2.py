#!/usr/bin/env python3
"""Build a synthetic CR2 file from the synthetic TIFF.

CR2 is a TIFF with a 4-byte Canon header after the TIFF header. All absolute
offsets shift by four bytes, and both IFD0 pointers (offset 4 and offset 12)
must point at the relocated IFD0.
"""

import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT / "synthetic" / "base.tif"
TARGET = ROOT / "synthetic" / "cr2_sample.cr2"
LE = "<"


def ifd_offsets(data, start):
    seen, queue, result = set(), [start], []
    while queue:
        offset = queue.pop()
        if offset in seen or offset + 2 > len(data):
            continue
        seen.add(offset)
        result.append(offset)
        count = struct.unpack_from(LE + "H", data, offset)[0]
        for index in range(count):
            entry = offset + 2 + index * 12
            tag = struct.unpack_from(LE + "H", data, entry)[0]
            if tag in (0x8769, 0x8825, 0xA005, 0x014A):
                queue.append(struct.unpack_from(LE + "I", data, entry + 8)[0])
        next_offset = struct.unpack_from(LE + "I", data, offset + 2 + count * 12)[0]
        if next_offset:
            queue.append(next_offset)
    return result


def main():
    source = SOURCE.read_bytes()
    if source[:4] != b"II*\x00":
        raise SystemExit("expected a little-endian TIFF source")
    ifd0 = struct.unpack_from(LE + "I", source, 4)[0]
    offsets = ifd_offsets(source, ifd0)

    data = bytearray(source)
    data[8:8] = b"CR\x02\x00"
    struct.pack_into(LE + "I", data, 12, ifd0 + 4)
    struct.pack_into(LE + "I", data, 4, ifd0 + 4)

    type_sizes = {1: 1, 2: 1, 3: 2, 4: 4, 5: 8, 6: 1, 7: 1, 8: 2, 9: 4, 10: 8, 11: 4, 12: 8}
    for offset in offsets:
        relocated = offset + 4
        count = struct.unpack_from(LE + "H", data, relocated)[0]
        for index in range(count):
            entry = relocated + 2 + index * 12
            tag, typ, count_value = struct.unpack_from(LE + "HHI", data, entry)
            total = type_sizes.get(typ, 1) * count_value
            if total > 4:
                value = struct.unpack_from(LE + "I", data, entry + 8)[0]
                struct.pack_into(LE + "I", data, entry + 8, value + 4)
            if tag in (0x8769, 0x8825, 0xA005):
                value = struct.unpack_from(LE + "I", data, entry + 8)[0]
                struct.pack_into(LE + "I", data, entry + 8, value + 4)
        next_offset = struct.unpack_from(LE + "I", data, relocated + 2 + count * 12)[0]
        if next_offset:
            struct.pack_into(LE + "I", data, relocated + 2 + count * 12, next_offset + 4)

    # CR2 reinterprets the TIFF strip as the preview image. Embed a real JPEG
    # there so `-b -PreviewImage` has extractable bytes, like a camera CR2.
    preview = ROOT / "synthetic" / "base.jpg"
    if preview.exists():
        jpeg = preview.read_bytes()
        relocated = ifd0 + 4
        count = struct.unpack_from(LE + "H", data, relocated)[0]
        start = length = None
        for index in range(count):
            entry = relocated + 2 + index * 12
            tag = struct.unpack_from(LE + "H", data, entry)[0]
            if tag == 0x0111:
                start = struct.unpack_from(LE + "I", data, entry + 8)[0]
            elif tag == 0x0117:
                length = struct.unpack_from(LE + "I", data, entry + 8)[0]
        if start is not None and length is not None:
            if len(jpeg) > length:
                raise SystemExit("base.jpg does not fit into the preview strip")
            data[start : start + length] = b"\0" * length
            data[start : start + len(jpeg)] = jpeg

    TARGET.write_bytes(data)
    print(f"wrote {TARGET} ({len(data)} bytes, {len(offsets)} IFDs)")


if __name__ == "__main__":
    main()
