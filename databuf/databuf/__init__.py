import collections

################################### Codd's primitives (expanded) ///////////////////////////////////

# A relation as we'd know it.  Primary key is the key of the table, and the schema is as defined.
# The table returns everything.  Use projection to pick a subset of fields.
Relation = collections.namedtuple('Relation', ('table',))

# A multiset of all objects in the provided relations.  Must be union-compatible.
SetUnion = collections.namedtuple('SetUnion', ('relation_a', 'relation_b',))

# A multiset of objects in relation_a that are not in relation_b.  Must be union-compatible.
SetDifference = collections.namedtuple('SetDifference', ('relation_a', 'relation_b'))

# A multiset of the cartesian product of the objects.  Must be attribute-disjoint.
CartesianProduct = collections.namedtuple('CartesianProduct', ('relation_a', 'relation_b'))

# Projection restricts the relation to only the listed proto paths.
Projection = collections.namedtuple('Projection', ('relation', 'paths'))

# Selection filters the relation to only those tuples that match the provided predicate.
Selection = collections.namedtuple('Selection', ('relation', 'predicate'))

# Rename remaps one type into another.
Rename = collections.namedtuple('Rename', ('relation', 'path_mapping'))

# A multisetof objects in all provided relations.  Must be union-compatible.
# Derivable from SetUnion and SetDifference; included for performance.
SetIntersection = collections.namedtuple('SetIntersection', ('relation_a', 'relation_b',))

############################################### Joins //////////////////////////////////////////////

# Natural is the set of all combinations that are equal on common attributes.
# This would be the same as renaming shared fields to avoid conflict, doing a cartesian, and then
# returning only those rows whos originally conflicting/same field names that are equal.
NaturalJoin = collections.namedtuple('NaturalJoin', ('relation_a', 'relation_b'))

# ThetaJoin is the Cartesian product of two relations, selecting only those rows matching predicate.
ThetaJoin = collections.namedtuple('ThetaJoin', ('relation_a', 'relation_b', 'predicate'))

# SemiJoin is like the natural join, except that relation_b is not returned.
SemiJoin = collections.namedtuple('SemiJoin', ('relation_a', 'relation_b'))

# An AntiJoin returns only those rows in relation_a for which there is no row in relation_b.
# SetUnion(SemiJoin(relation_a, relation_b), AntiJoin(relation_a, relation_b)) == relation_a.
AntiJoin = collections.namedtuple('AntiJoin', ('relation_a', 'relation_b'))

############################################ Expressions ///////////////////////////////////////////

# Predicates and expressions
UNARY_OPS = ('not',)
Unary = collections.namedtuple('Unary', ('oper', 'expr'))

BINARY_OPS = ('and', 'or', '+', '-', '*', '/', '%')
Binary = collections.namedtuple('Binary', ('oper', 'expr1', 'expr2'))

########################################### KeyValueStore //////////////////////////////////////////

class KeyValueStore:
    'KeyValueStore is an underlying storage system that uses tuple keys and byte values.'

    def __init__(self):
        self._storage = {}

    def get(self, key):
        assert isinstance(key, tuple)
        return self._storage.get(key)

    def put(self, key, value):
        assert isinstance(key, tuple)
        self._storage[key] = value

    def scan(self, prefix):
        assert isinstance(prefix, tuple)
        return sorted((key, value) for key, value in self._storage.items() if key[:len(prefix)] == prefix)
