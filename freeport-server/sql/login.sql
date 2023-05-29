insert into crates_index.tokens (user_id, token_hash)
select users.id, crypt($3, gen_salt('bf'))
from crates_index.users
where users.username = $1
  and users.password_hash = crypt($2, users.password_hash)
returning tokens.id