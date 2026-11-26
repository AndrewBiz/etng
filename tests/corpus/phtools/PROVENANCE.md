# phtools fixtures — provenance

Source: `https://github.com/AndrewBiz/phtools`  
Commit: `8e3f18c` (2024-09-07)  
Author: Andrey Bizyaev (ANB)  
License: MIT (see the upstream `LICENSE.txt`, `Copyright (c) 2012 ANDREW BIZYAEV`)

Copied unchanged from `features/media/` for etng write/date tests:

- `dates/*.JPG` — date-source fixtures (`DateTimeOriginal`, IPTC/XMP dates,
  FileModifyDate, all_dates);
- `iphone/IMG_0887_no_dto_cd.jpg` — no DateTimeOriginal/CreateDate;
- `tags/tag_dto*.JPG`, `tags/tag_cd*.JPG` — existing/missing EXIF dates;
- `tags/tag_{city,country,creator,keywords,location,state,collections,copyright}*.JPG`
  — MWG tag round-trip fixtures.

These files are intentionally excluded from `xtask compare`: they exercise
missing-tag insertion and raw Sony MakerNotes, not the golden read corpus.
