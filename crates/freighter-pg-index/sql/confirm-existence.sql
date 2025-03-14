select cv.yanked, cv.cksum
from crates
         join crate_versions cv on crates.id = cv.crate
where crates.name = $1
  and cv.version = $2
  and crates.registry = ''
