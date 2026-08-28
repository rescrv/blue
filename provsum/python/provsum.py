"""Reference implementation of provsum, byte-compatible with the Rust crate.

Digest = N[X] provenance polynomial evaluated at pinned points in F_p,
p = 2^61 - 1, two lanes.  See src/lib.rs for the construction.
"""
from __future__ import annotations

from dataclasses import dataclass

LANES = 2
P = (1 << 61) - 1
MASK64 = (1 << 64) - 1
NO_SLOT = 0xFFFFFFFF
SEED = 0x70726F767375_6D31  # "provsum1"
DIGEST_BYTES = LANES * 8 + 2


def mix(z: int) -> int:
    z = (z + 0x9E3779B97F4A7C15) & MASK64
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK64
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK64
    return z ^ (z >> 31)


def site_from_label(label: str) -> bytes:
    """Test/example convenience only; assign real GUIDs in production."""
    s = mix(0x6C6162656C5F7369)
    data = label.encode()
    for i in range(0, max(len(data), 1), 8):
        chunk = data[i:i + 8]
        if not chunk and i > 0:
            break
        w = int.from_bytes(chunk.ljust(8, b"\0"), "little")
        s = mix(s ^ w ^ (i // 8))
    return s.to_bytes(8, "little") + mix(s).to_bytes(8, "little")


def point(site: bytes, slot: int, lane: int) -> int:
    assert len(site) == 16
    s = mix(SEED ^ (lane << 32) ^ slot)
    for i in (0, 8):
        s = mix(s ^ int.from_bytes(site[i:i + 8], "little"))
    while True:
        r = s % P
        if r != 0:
            return r
        s = mix(s)


@dataclass(frozen=True)
class Digest:
    lanes: tuple[int, ...]
    depth: int = 0

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Digest) and self.lanes == other.lanes

    def __hash__(self) -> int:
        return hash(self.lanes)

    @staticmethod
    def zero() -> "Digest":
        return Digest((0,) * LANES, 0)

    @staticmethod
    def _points(site: bytes, slot: int) -> tuple[int, ...]:
        return tuple(point(site, slot, lane) for lane in range(LANES))

    @staticmethod
    def origin(site: bytes) -> "Digest":
        return Digest(Digest._points(site, NO_SLOT), 1)

    def wrap(self, site: bytes) -> "Digest":
        r = Digest._points(site, NO_SLOT)
        return Digest(
            tuple((r[i] * self.lanes[i]) % P for i in range(LANES)),
            min(self.depth + 1, 0xFFFF),
        )

    def merge(self, other: "Digest") -> "Digest":
        return Digest(
            tuple((self.lanes[i] + other.lanes[i]) % P for i in range(LANES)),
            max(self.depth, other.depth),
        )

    @staticmethod
    def pick(site: bytes, ranked: list["Digest"]) -> "Digest":
        acc = Digest.zero()
        for slot, d in enumerate(ranked):
            r = Digest._points(site, slot)
            term = Digest(
                tuple((r[i] * d.lanes[i]) % P for i in range(LANES)),
                min(d.depth + 1, 0xFFFF),
            )
            acc = acc.merge(term)
        return acc

    @staticmethod
    def pick2(site: bytes, returned: "Digest", discarded: "Digest") -> "Digest":
        return Digest.pick(site, [returned, discarded])

    def to_bytes(self) -> bytes:
        return b"".join(l.to_bytes(8, "little") for l in self.lanes) + self.depth.to_bytes(2, "little")

    @staticmethod
    def from_bytes(b: bytes) -> "Digest":
        assert len(b) == DIGEST_BYTES
        lanes = tuple(int.from_bytes(b[i * 8:i * 8 + 8], "little") for i in range(LANES))
        assert all(l < P for l in lanes)
        return Digest(lanes, int.from_bytes(b[LANES * 8:], "little"))

    def to_hex(self) -> str:
        return "".join(f"{l:016x}" for l in self.lanes)


if __name__ == "__main__":
    # Emit the same vectors as examples/vectors.rs for cross-checking.
    a, b, c = (site_from_label(x) for x in ("a", "b", "c"))
    w1, w2, ch = (site_from_label(x) for x in ("w1", "w2", "choose"))
    vectors = {
        "origin_raw_bytes": Digest.origin(bytes(range(16))),
        "origin_a": Digest.origin(a),
        "wrap": Digest.origin(a).wrap(w1),
        "merge": Digest.origin(a).merge(Digest.origin(b)),
        "pick_ab": Digest.pick2(ch, Digest.origin(a), Digest.origin(b)),
        "pick_ba": Digest.pick2(ch, Digest.origin(b), Digest.origin(a)),
        "nested": Digest.pick2(
            ch,
            Digest.origin(a).wrap(w1).merge(Digest.origin(b).wrap(w1)),
            Digest.origin(c).wrap(w2),
        ).wrap(w2),
    }
    for k, v in vectors.items():
        print(f"{k} {v.to_hex()} {v.depth}")
