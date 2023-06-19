select freighter_crate_owners.id
from freighter_tokens
         join freighter_crate_owners on freighter_tokens.user_id = freighter_crate_owners.user_id
where token_hash = crypt($1, freighter_tokens.token_hash)
  and freighter_crate_owners.crate = $2