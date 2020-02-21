# rusty_mara
reimplementation from mara in rust

# Useful fur debugging
- [gdb output formats](https://sourceware.org/gdb/current/onlinedocs/gdb/Output-Formats.html#Output-Formats)
- [dgb memory view](https://sourceware.org/gdb/current/onlinedocs/gdb/Memory.html#Memory)

## Read addresses
cmds:
  - **memory** (x): read memory
  - **expression** (expr, e): evaluate expression

formats:
  - **t** binary
  - **x** hex
  - **u** unsigned int

useful cmds:
- **single byte:** x/tb address
- **N bytes:** x/Ntb address *//useful for codeblocks*