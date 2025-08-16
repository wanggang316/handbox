<script lang="ts">
  import ChatHeaderView from '$lib/components/chat/ChatHeaderView.svelte';
  import ChatContentView from '$lib/components/chat/ChatContentView.svelte';
  import ChatInputView from '$lib/components/chat/ChatInputView.svelte';
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import { sidebarOpen } from '$lib/stores/ui';

  let sessionId = $state('');
  let messageInput = $state('');
  let selectedModel = $state('DeepSeek R1');

  // 会话数据映射
  const sessionsData: Record<string, { title: string; content: string }> = {
    "2": {
      title: "Claude Code 使用指南",
      content: `Claude Code 是一个强大的AI编程助手，以下是使用指南：\n\n• 代码生成：描述你想要的功能，Claude会生成相应代码\n• 代码解释：粘贴代码片段，Claude会详细解释其工作原理\n• 调试帮助：描述遇到的问题，Claude会提供解决方案\n• 最佳实践：询问编程最佳实践，获得专业建议`
    },
    "3": {
      title: "经典贪食蛇网页游戏",
      content: `贪食蛇游戏开发要点：\n\n• HTML5 Canvas：用于游戏画面渲染\n• JavaScript：实现游戏逻辑和控制\n• 碰撞检测：检测蛇头与边界、食物、身体的碰撞\n• 游戏循环：使用 requestAnimationFrame 创建流畅动画\n• 得分系统：记录并显示玩家得分`
    },
    "4": {
      title: "Python npx 命令行工具介绍",
      content: `NPX 是 Node.js 的包执行器，Python 中的类似工具：\n\n• pipx：专门用于安装和运行 Python 应用程序\n• pip：Python 包管理器，可以安装命令行工具\n• poetry：现代Python依赖管理和打包工具\n• conda：科学计算环境管理器`
    },
    "5": {
      title: "今日 AI 新闻热点汇总",
      content: `AI 领域最新动态：\n\n• OpenAI 发布新版本 ChatGPT，提升推理能力\n• Google DeepMind 在蛋白质折叠预测方面取得突破\n• 微软推出 Copilot 企业版，集成更多办公场景\n• Meta 开源新的多模态大语言模型\n• 自动驾驶技术在城市道路测试中表现优异`
    },
    "6": {
      title: "推荐股票学习资料",
      content: `股票投资学习资源推荐：\n\n• 书籍：《聪明的投资者》、《股票大作手回忆录》\n• 网站：雪球、东方财富、同花顺\n• 课程：财经类大学课程、在线投资教育平台\n• 工具：股票分析软件、财务报表分析工具\n• 提醒：投资有风险，入市需谨慎`
    },
    "7": {
      title: "Go 语言学习资料推荐",
      content: `Go 语言学习路径：\n\n• 官方教程：Go Tour 交互式学习\n• 经典书籍：《Go程序设计语言》、《Go语言实战》\n• 实践项目：Web服务、命令行工具、微服务\n• 开源项目：Docker、Kubernetes、Prometheus\n• 社区资源：Go官方博客、Gopher社区`
    },
    "8": {
      title: "小猫照片编辑生成",
      content: `AI图像生成和编辑工具：\n\n• Stable Diffusion：开源图像生成模型\n• DALL-E：OpenAI的图像生成工具\n• Midjourney：高质量艺术图像生成\n• Photoshop AI：Adobe的AI辅助编辑功能\n• 在线工具：Canva、Figma等设计平台的AI功能`
    }
  };

  // 从 URL 参数获取会话 ID
  onMount(() => {
    const urlParams = $page.url.searchParams;
    sessionId = urlParams.get('id') || '';
    console.log('Current session ID:', sessionId);
  });

  // 监听 URL 变化
  $effect(() => {
    const urlParams = $page.url.searchParams;
    sessionId = urlParams.get('id') || '';
    console.log('Session ID changed to:', sessionId);
  });

  // 根据 sessionId 获取当前会话数据
  const currentSession = $derived(sessionId ? sessionsData[sessionId] || null : null);
  
  // 默认消息（当没有选择会话时显示）
  const defaultMessage = {
    content: `欢迎使用 HandBox！\n\n• 从左侧选择一个会话开始对话\n• 或者点击"New chat"创建新的对话\n• 使用搜索功能快速找到历史对话`,
    actions: [
      { icon: 'copy', tooltip: 'Copy' },
      { icon: 'edit', tooltip: 'Edit' },
      { icon: 'refresh', tooltip: 'Refresh' },
      { icon: 'share', tooltip: 'Share' }
    ]
  };

  const currentMessage = $derived(currentSession ? {
    content: currentSession.content,
    actions: [
      { icon: 'copy', tooltip: 'Copy' },
      { icon: 'edit', tooltip: 'Edit' },
      { icon: 'refresh', tooltip: 'Refresh' },
      { icon: 'share', tooltip: 'Share' }
    ]
  } : defaultMessage);

  // 处理消息发送
  function handleSendMessage(message: string) {
    console.log('Sending message:', message);
    // 这里可以添加发送消息的逻辑
  }

  // 处理模型切换
  function handleModelChange(model: string) {
    console.log('Model changed to:', model);
    // 这里可以添加模型切换的逻辑
  }
</script>

<!-- 聊天页面 -->
<div class="flex-1 flex flex-col">
  <ChatHeaderView 
    {sessionId} 
    title={currentSession ? currentSession.title : 'HandBox - AI 助手'}
    sidebarOpen={$sidebarOpen}
  />
  
  <ChatContentView 
    message={currentMessage} 
  />
  
  <div class="px-4 pb-4">
    <ChatInputView 
      bind:messageInput={messageInput}
      bind:selectedModel={selectedModel}
      onSendMessage={handleSendMessage}
      onModelChange={handleModelChange}
    />
  </div>
</div>