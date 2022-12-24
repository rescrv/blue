TEMPLATE = '''            #[test]
            fn num_keys_{num_keys}_key_bytes_{key_bytes}_value_bytes_{value_bytes}_num_seeks_{num_seeks}_seek_distance_{seek_distance}_prev_probability_{prev_probability_str}() {{
                let name = stringify!($name).to_string() + "::" + "num_keys_{num_keys}_key_bytes_{key_bytes}_value_bytes_{value_bytes}_num_seeks_{num_seeks}_seek_distance_{seek_distance}_prev_probability_{prev_probability_str}";
                let config = crate::guacamole::FuzzerConfig {{
                    num_keys: {num_keys},
                    key_bytes: {key_bytes},
                    value_bytes: {value_bytes},
                    num_seeks: {num_seeks},
                    seek_distance: {seek_distance},
                    prev_probability: {prev_probability},
                }};
                crate::guacamole::fuzzer(&name, config, $builder);
            }}
'''

for num_keys in (1, 10, 100, 1000):
    for key_bytes in (1, 16, 256, 4096, 65536):
        for value_bytes in (0, 1, 16, 256, 4096, 65536, 1048576, 16777216):
            for num_seeks in (10000,):
                for seek_distance in (1, 10, 100, 1000):
                    for prev_probability in (0.0, 0.125, 0.25, 0.5):
                        prev_probability_str = '{}'.format(prev_probability).replace('.', '_')
                        print(TEMPLATE.format(
                            num_keys=num_keys,
                            key_bytes=key_bytes,
                            value_bytes=value_bytes,
                            num_seeks=num_seeks,
                            seek_distance=seek_distance,
                            prev_probability=prev_probability,
                            prev_probability_str=prev_probability_str,
                        ))
