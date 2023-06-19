insert into freighter_crate_owners (user_id, crate)
values ($1, $2)
on conflict do nothing;