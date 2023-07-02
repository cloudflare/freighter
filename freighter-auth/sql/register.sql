insert into freighter_users (username, password_hash)
values ($1, crypt($2, gen_salt('bf')))
returning id;
