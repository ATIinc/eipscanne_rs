use bincode::{deserialize, serialize};

use eipscanne_rs::samples::{ChildStruct, MyStruct};

#[test]
fn test_serialize_deserialize() {
    let my_struct = MyStruct {
        byte_field: 10,
        int_field: -100,
        double_field: 3.14159,
        ubyte_field: 255,
        byte_array: [1, 2, 3, 4, 5, 6],
        child: ChildStruct {
            id: 1234,
            description: String::from("A child struct example"),
        },
    };

    // Serialize
    let serialized = serialize(&my_struct).expect("Serialization failed");

    // Deserialize
    let deserialized: MyStruct = deserialize(&serialized).expect("Deserialization failed");

    // Assert equality
    assert_eq!(my_struct, deserialized);
}
