insert into crate_versions (crate, version, cksum, yanked, links)
values ($1, $2, $3, $4, $5)
returning id
