.extern fred_ring3
.extern fred_ring0
.global fred_entry_point

.section .text
.align 4096

fred_entry_point:
fred_ring3_entry_stub:
  jmp fred_ring3

.skip (256 - (. - fred_ring3_entry_stub))

fred_ring0_entry_stub:
  jmp fred_ring0
