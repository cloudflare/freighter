insert into crates_index.features (crate_version, name, values)
values ($1, $2, $3)
returning id