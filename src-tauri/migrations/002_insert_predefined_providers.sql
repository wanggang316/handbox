-- Insert predefined provider templates as initial data
-- These are the default providers that users can choose from

INSERT OR IGNORE INTO providers (
    id, 
    name, 
    provider_type, 
    base_url, 
    status, 
    enabled, 
    last_probe_at, 
    probe_result, 
    created_at, 
    updated_at
) VALUES 
    (
        'openai-default',
        'OpenAI',
        'openai',
        'https://api.openai.com/v1',
        'idle',
        0,
        NULL,
        NULL,
        strftime('%s', 'now') * 1000,
        strftime('%s', 'now') * 1000
    ),
    (
        'anthropic-default',
        'Anthropic',
        'anthropic',
        'https://api.anthropic.com',
        'idle',
        0,
        NULL,
        NULL,
        strftime('%s', 'now') * 1000,
        strftime('%s', 'now') * 1000
    ),
    (
        'google-default',
        'Google AI',
        'google',
        'https://generativelanguage.googleapis.com/v1',
        'idle',
        0,
        NULL,
        NULL,
        strftime('%s', 'now') * 1000,
        strftime('%s', 'now') * 1000
    ),
    (
        'deepseek-default',
        'DeepSeek',
        'deepseek',
        'https://api.deepseek.com',
        'idle',
        0,
        NULL,
        NULL,
        strftime('%s', 'now') * 1000,
        strftime('%s', 'now') * 1000
    ),
    (
        'openrouter-default',
        'OpenRouter',
        'openrouter',
        'https://openrouter.ai/api/v1',
        'idle',
        0,
        NULL,
        NULL,
        strftime('%s', 'now') * 1000,
        strftime('%s', 'now') * 1000
    );