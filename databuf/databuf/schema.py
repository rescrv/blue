import collections
import pprint
import re
import sys

import ply.lex
import ply.yacc

__all__ = ['Field', 'Reserved', 'Object', 'TimeSeries', 'KV', 'Table', 'parse_schema']

############################################# datatype /////////////////////////////////////////////
# I like 8-letter names where possible.  Hence the lack of a space for data type.

Field = collections.namedtuple('Field', ('number', 'name', 'datatype', 'repeated', 'breakout'))
Reserved = collections.namedtuple('Reserved', ('number',))
Object = collections.namedtuple('Object', ('fields',))
TimeSeries = collections.namedtuple('TimeSeries', ('datatype',))

KV = collections.namedtuple('KV', ('name',))
Table = collections.namedtuple('Table', ('name', 'key', 'fields'))

############################################### lexer //////////////////////////////////////////////

reserved = {
    'KV': 'KV',
    'table': 'TABLE',
    'reserve': 'RESERVE',
    'repeated': 'REPEATED',
    'breakout': 'BREAKOUT',

    'int32': 'INT32',
    'int64': 'INT64',
    'uint32': 'UINT32',
    'uint64': 'UINT64',
    'sint32': 'SINT32',
    'sint64': 'SINT64',
    'bool': 'BOOL',
    'fixed32': 'FIXED32',
    'fixed64': 'FIXED64',
    'sfixed32': 'SFIXED32',
    'sfixed64': 'SFIXED64',
    'float': 'FLOAT',
    'double': 'DOUBLE',
    'bytes': 'BYTES',
    'bytes32': 'BYTES32',
    'string': 'STRING',

    'timeseries': 'TIMESERIES',
    'object': 'OBJECT',
}

tokens = (
    'COMMA',
    'EQUALS',
    'LPAREN',
    'RPAREN',
    'LBRACE',
    'RBRACE',
    'SEMICOLON',
    'COMMENT',
    'NUMBER',
    'ATOM',
) + tuple(reserved.values())

t_ignore = " \t"

t_COMMA = ','
t_EQUALS = '='
t_LPAREN = '\\('
t_RPAREN = '\\)'
t_LBRACE = '{'
t_RBRACE = '}'
t_SEMICOLON = ';'

def t_COMMENT(t):
    r'\#.*'
    pass

def t_NUMBER(t):
    r'[1-9][0-9]*'
    t.value = int(t.value)
    return t

def t_ATOM(t):
    r'[a-zA-Z_][-a-zA-Z0-9_]*'
    t.type = reserved.get(t.value, 'ATOM')
    return t

def t_newline(t):
    r'\n+'
    t.lexer.lineno += t.value.count("\n")

def t_error(t):
    print("Illegal character '%s'" % t.value[0])
    t.lexer.skip(1)
    raise RuntimeError("get me out of here")

############################################## parser //////////////////////////////////////////////

def p_databuf(t):
    '''databuf : definition
    '''
    t[0] = [t[1]]

def p_databuf_list(t):
    '''databuf : databuf definition
               | databuf definition COMMENT
    '''
    t[0] = t[1] + [t[2]]

def p_definition(t):
    '''definition : key_value
                  | table
    '''
    # NOTE(rescrv):  If you extend a definition to be more than a table, look out.
    t[0] = t[1]

def p_key_value(t):
    '''key_value : TABLE ATOM KV SEMICOLON
    '''
    t[0] = KV(name=t[2])

def p_table(t):
    '''table : TABLE ATOM LPAREN atom_list RPAREN LBRACE object_body RBRACE
    '''
    name = t[2]
    key = t[4]
    fields = t[7]
    field_names = [f.name for f in fields if isinstance(f, Field)]
    for k in key:
        assert k in field_names
    t[0] = Table(name=name, key=key, fields=fields)

def p_object_body_base(t):
    '''object_body : object_decl SEMICOLON
    '''
    t[0] = (t[1],)

def p_object_body_list(t):
    '''object_body : object_body object_decl SEMICOLON
    '''
    t[0] = t[1] + (t[2],)

def p_table_decl(t):
    '''object_decl : field
                   | reservation
    '''
    t[0] = t[1]

def p_field(t):
    '''field : field_options datatype ATOM EQUALS NUMBER
    '''
    t[0] = Field(datatype=t[2], number=t[5], name=t[3],
            repeated='repeated' in t[1],
            breakout='breakout' in t[1])

def p_field_options_base(t):
    '''field_options :
    '''
    t[0] = ()

def p_field_options_repeated(t):
    '''field_options : field_options REPEATED
    '''
    t[0] = t[1] + ('repeated',)

def p_field_options_breakout(t):
    '''field_options : field_options BREAKOUT
    '''
    t[0] = t[1] + ('breakout',)

def p_table_reserve(t):
    '''reservation : RESERVE NUMBER
    '''
    t[0] = Reserved(number=t[2])

def p_atom_list_base(t):
    '''atom_list : ATOM
    '''
    t[0] = (t[1],)

def p_atom_list_list(t):
    '''atom_list : atom_list COMMA ATOM
    '''
    t[0] = t[1] + (t[3],)

def p_datatype_timeseries(t):
    '''datatype : TIMESERIES datatype
    '''
    t[0] = TimeSeries(datatype=t[2])

def p_datatype_object(t):
    '''datatype : OBJECT LBRACE object_body RBRACE
    '''
    t[0] = Object(fields=t[3])

def p_datatype_terminal(t):
    '''datatype : INT32
                | INT64
                | UINT32
                | UINT64
                | SINT32
                | SINT64
                | BOOL
                | FIXED32
                | FIXED64
                | SFIXED32
                | SFIXED64
                | FLOAT
                | DOUBLE
                | BYTES
                | BYTES32
                | STRING
    '''
    t[0] = t[1]

def p_error(t):
    if t is not None:
        sys.stderr.write("Syntax error at '%s' on line %d.\n" % (t.value, t.lexer.lineno))
    else:
        sys.stderr.write("Syntax error.\n")
    raise RuntimeError("get me out of here")

############################################### misc ///////////////////////////////////////////////

def parse_schema(contents):
    lexer = ply.lex.lex(reflags=re.UNICODE)
    lexer.lineno = 1
    parser = ply.yacc.yacc(debug=0, write_tables=0)
    return parser.parse(contents, lexer=lexer)

if __name__ == '__main__':
    parse_schema('''
table Boundaries KV;

table SSTs (id) {
    bytes32 id = 1;
    bytes first = 2;
    bytes last = 3;
    bytes32 sha256 = 4;
    bytes32 setsum = 5;
    uint64 num_records = 14;
    reserve 15;

    # We could dedupe fields across these verification objects, but then we don't get the property that verification
    # scrubbers write under separate tags, which makes it a lot easier to spot un-error'd fields when hand-fixing any
    # errors that could go wrong.
    timeseries object {
        bytes32 binary = 6;
        bytes32 output = 7;
        breakout bool mute = 8;
    } verify_sha256 = 9;

    timeseries object {
        bytes32 binary = 10;
        bytes32 output = 11;
        breakout bool mute = 12;
    } verify_setsum = 13;

    # ... more verifiers go here.  Use this is a place to output the results of the verifier to be checked by other
    # tools.  For example, the verify_setsum output is scrubbed to match the SST's inbuilt output, but the way scrubbing
    # works, there are two verifier binaries, one that scrubs by picking the setsum from the footer and one that
    # computes it.  Maybe there's a third way of doing things that gets discovered, but it can build into the existing
    # verifiers or another set of verification routines.
}

table Compactions(id) {
    bytes32 id = 1;
    repeated bytes32 ssts_input = 2;
    repeated bytes32 ssts_output = 3;
    bytes32 collected = 4;
    breakout bool verified = 5;

    # ... verifiers output here and verified gets flipped when it's permissible to collect the compaction.
}
''')
