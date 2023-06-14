import math

print('K = &[')
for i in range(64):
    sep = '&['
    s = ''
    for j in range(i + 1):
        s += sep + str(math.comb(i, j))
        sep = ', '
    s += '],'
    print(s)
print(']');

print('L = &[')
for i in range(64):
    print(math.ceil(math.log2(math.comb(63, i))))
print(']');
