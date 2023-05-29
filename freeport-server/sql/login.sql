insert into crates_index.tokens (user_id, token_hash)
select users.id, crates_index.crypt($3, crates_index.gen_salt('bf'))
from crates_index.users
where users.username = $1
  and users.password_hash = crates_index.crypt($2, users.password_hash)
returning tokens.id