insert into freighter_users (username)
values ($1)
returning id;
