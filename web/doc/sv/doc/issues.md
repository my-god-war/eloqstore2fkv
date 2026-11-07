# Granska poster

`issue/` är granskningsnämnden under migreringen:

- `new/`: Inlägg för nya kommentarer.
- `todo/`: Antagen, väntar på utveckling.
- `done/`: Utvecklat och har testbevis, i väntan på granskning.
- `reviewed/`: Arkiv granskat.
- `reject/`: Om det inte antas, vänligen ange skälen.

Se `issue/new.md`, `issue/run.md`, `issue/review.md` för processen.

Granskningen fokuserar på beteendeskillnader, formatkompatibilitet, prestanda och kodstruktur mellan fkv och C++ implementeringar. En åsikt per
filer för att undvika massflyttningar.