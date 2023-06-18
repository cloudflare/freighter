update crates_index.crate_versions cv
set yanked = $3
from crates_index.crates c
where c.name = $1
  and cv.crate = c.id
  and cv.version = $2
returning c.name, cv.version, cv.yanked;