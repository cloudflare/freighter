select fu.username
from freighter_tokens
         join freighter_users fu on fu.id = freighter_tokens.user_id
where freighter_tokens.token_hash = crypt($1, freighter_tokens.token_hash);
