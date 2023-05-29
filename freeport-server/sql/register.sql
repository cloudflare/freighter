insert into crates_index.users (username, password_hash)
values ($1, crates_index.crypt($2, crates_index.gen_salt('bf')))
returning id;