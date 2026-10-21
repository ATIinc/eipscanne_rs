# eds_parser_derive

The derives of [`eds_parser`](../eds_parser/README.md)'s two reading traits.

## FromEntry

Reads a struct with named fields from an EDS entry, each member from the next field in file
order; a `Vec` member takes the rest of the entry. Used by `ParamEntry`, `AssemblyEntry`,
`AssemblyMember` and `ConnectionEntry`.

## FromField

Reads a `bilge` bitfield or enum from one field: the field as the type's backing integer (`u8` for
`#[bitsize(8)]`), converted with `FromBits`. Used by `ParamDescriptor`, `CipDataType` and the two
connection masks.
