insert into crates (name, created_at, updated_at)
values ($1, current_timestamp, current_timestamp)
on conflict (name, registry) do update set updated_at = current_timestamp
returning *;
