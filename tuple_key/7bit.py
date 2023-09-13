'''Create 7-bit coded bytes from a listing of 8-bit integers.
'''

import sys

args = list(map(int, sys.argv[1:]))
assert all((0 <= x < 256 for x in args))
bits = ''.join([bin(x)[2:].rjust(8, '0') for x in args])
while len(bits) % 7 != 0:
    bits += '0'
while bits:
    x = int(bits[:7], 2)
    if bits[7:]:
        print('0b{:08b}'.format((x << 1) | 1))
    else:
        print('0b{:08b}'.format(x << 1))
    bits = bits[7:]
