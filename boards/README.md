# About

This directory contains structured board description (sbd) files that Ariel OS is
using to generate the board support from.

## Generating board support

1. Make sure `sbd-gen` is installed:

    cargo install sbd-gen

2. Use the wrapper script to generate/update the `ariel-os-boards` crate from the
sbd files:

    pwsh ./scripts/generate-ariel-boards.ps1

This wrapper still calls `sbd-gen`, but it also preserves Ariel OS local board
extensions that are not part of upstream SBD yet, such as the `modem` subtree in
[boards/st-stm32f427vg.yaml](boards/st-stm32f427vg.yaml).

If you are only working with pure upstream-compatible SBD files, calling `sbd-gen`
directly still works:

    sbd-gen generate-ariel --mode update boards -o src/ariel-os-boards

See [sbd-gen][sbd-gen] for more information.

[sbd-gen]: https://github.com/ariel-os/sbd
