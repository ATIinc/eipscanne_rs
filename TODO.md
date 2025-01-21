# TODO

- [x] Figure out how to remove the `byte_size()` method from the various structs
    * Solution: Calculate the size of the packet length at writing
- [ ] Figure out how to calculate the size of the MessageRouter<T> without serializing it