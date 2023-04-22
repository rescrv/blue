import sys

args = list(map(int, sys.argv[1:]))
print(args)
bits = ''.join([bin(x)[2:].rjust(8, '0') for x in args])
bits += '0' * (7 - len(bits) % 7)
while bits:
    x = int(bits[:7], 2)
    if bits[7:]:
        print('0x{:02x}'.format((x << 1) | 1))
    else:
        print('0x{:02x}'.format(x << 1))
    bits = bits[7:]
