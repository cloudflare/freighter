insert into freighter_tokens (user_id, token_hash)
select freighter_users.id, crypt($3, gen_salt('bf'))
from freighter_users
where freighter_users.username = $1
  and freighter_users.password_hash = crypt($2, freighter_users.password_hash)
returning freighter_tokens.id