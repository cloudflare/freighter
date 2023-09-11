insert into freighter_tokens (user_id, token_hash)
select freighter_users.id, crypt($2, gen_salt('bf'))
from freighter_users
where freighter_users.username = $1
returning freighter_tokens.id;
