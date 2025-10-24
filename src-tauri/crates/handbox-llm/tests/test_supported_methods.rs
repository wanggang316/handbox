use handbox_llm::types::{convert_endpoints_to_methods, to_snake_case};

#[test]
fn test_to_snake_case() {
    // 测试驼峰命名转换
    assert_eq!(to_snake_case("chatCompletions"), "chat_completions");
    assert_eq!(to_snake_case("generateContent"), "generate_content");
    assert_eq!(to_snake_case("embedContent"), "embed_content");
    assert_eq!(to_snake_case("simple"), "simple");
    assert_eq!(to_snake_case("UPPERCASE"), "u_p_p_e_r_c_a_s_e");

    // 测试空格转换
    assert_eq!(to_snake_case("Generate Content"), "generate_content");
    assert_eq!(to_snake_case("chat completions"), "chat_completions");
    assert_eq!(to_snake_case("Multiple  Spaces"), "multiple_spaces");
    assert_eq!(to_snake_case(" leading space"), "leading_space");
    assert_eq!(to_snake_case("trailing space "), "trailing_space_");
}

#[test]
fn test_convert_endpoints_to_methods_openai() {
    let endpoints = vec![
        "chatCompletions".to_string(),
        "embeddings".to_string(),
    ];
    let methods = convert_endpoints_to_methods(&endpoints, "openai");

    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0], "openai_chat_completions");
    assert_eq!(methods[1], "openai_embeddings");
}

#[test]
fn test_convert_endpoints_to_methods_google() {
    let endpoints = vec![
        "generateContent".to_string(),
        "streamGenerateContent".to_string(),
        "embedContent".to_string(),
    ];
    let methods = convert_endpoints_to_methods(&endpoints, "google");

    assert_eq!(methods.len(), 3);
    assert_eq!(methods[0], "google_generate_content");
    assert_eq!(methods[1], "google_stream_generate_content");
    assert_eq!(methods[2], "google_embed_content");
}

#[test]
fn test_convert_endpoints_to_methods_deepseek() {
    let endpoints = vec![
        "chatCompletions".to_string(),
    ];
    let methods = convert_endpoints_to_methods(&endpoints, "deepseek");

    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0], "deepseek_chat_completions");
}

#[test]
fn test_empty_endpoints() {
    let endpoints: Vec<String> = vec![];
    let methods = convert_endpoints_to_methods(&endpoints, "openai");

    assert_eq!(methods.len(), 0);
}
