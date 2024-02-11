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
