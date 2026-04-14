OUTPUT_FORMAT(elf64-x86-64)

ENTRY(kmain)

PHDRS
{
  text   PT_LOAD;
  rodata PT_LOAD;
  data   PT_LOAD;
}

SECTIONS
{
  . = 0xffffffff80000000;

  .text : {
    *(.text .text.*)
  } :text

  . = ALIGN(CONSTANT(MAXPAGESIZE));

  .rodata : {
    *(.rodata .rodata.*)

    *(.icon_data .icon_data.*)
  } :rodata

  . = ALIGN(CONSTANT(MAXPAGESIZE));

  .data : {
    *(.data .data.*)

    KEEP(*(.limine_requests_start))
    KEEP(*(.limine_requests))
    KEEP(*(.limine_requests_end))
  } :data

  .bss : {
    *(.bss .bss.*)
    *(COMMON)
  } :data

  /DISCARD/ : {
    *(.eh_frame*)
    *(.note .note.*)
  }
}
