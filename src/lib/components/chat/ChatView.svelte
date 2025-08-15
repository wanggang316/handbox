<script lang="ts">
  interface Props {
    sessionId?: string;
  }
  
  let { sessionId = '' }: Props = $props();
  
  let messageInput = $state('');
  let selectedModel = $state('DeepSeek R1');
  let showModelDropdown = $state(false);

  const models = ['DeepSeek R1', 'Claude 3.5 Sonnet', 'GPT-4', 'Gemini Pro'];

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

  function toggleModelDropdown() {
    showModelDropdown = !showModelDropdown;
  }

  function selectModel(model: string) {
    selectedModel = model;
    showModelDropdown = false;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      sendMessage();
    }
  }

  function sendMessage() {
    if (!messageInput.trim()) return;
    console.log('Sending message:', messageInput);
    messageInput = '';
  }
</script>

<!-- 顶部栏 -->
<header class="p-4 border-b border-gray-200 bg-white flex items-center justify-between">
  <div>
    <h1 class="text-base font-medium text-gray-900">
      {currentSession ? currentSession.title : 'HandBox - AI 助手'}
    </h1>
    {#if sessionId}
      <p class="text-xs text-gray-500 mt-1">会话 ID: {sessionId}</p>
    {/if}
  </div>
  <div class="flex gap-2">
    <button class="w-8 h-8 border border-gray-200 rounded-md flex items-center justify-center text-gray-500 hover:bg-gray-50 transition-colors" aria-label="打开面板">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M8 16H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v2m-6 12h8a2 2 0 0 0 2-2v-8a2 2 0 0 0-2-2h-8a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2z"/>
      </svg>
    </button>
    <button class="w-8 h-8 border border-gray-200 rounded-md flex items-center justify-center text-gray-500 hover:bg-gray-50 transition-colors" aria-label="编辑">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M11 5H6a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2v-5m-1.414-9.414a2 2 0 1 1 2.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/>
      </svg>
    </button>
  </div>
  </header>

<!-- 消息内容 -->
<div class="flex-1 overflow-y-auto">
  <div class="max-w-3xl mx-auto">
    <div class="text-[20px] leading-[1.73] text-black mb-4 whitespace-pre-line">
      {currentMessage.content}
    </div>
    <div class="flex gap-2">
      {#each currentMessage.actions as action}
        <button class="w-7 h-7 border border-gray-200 rounded flex items-center justify-center text-gray-500 hover:bg-gray-50 transition-colors" title={action.tooltip} aria-label={action.tooltip}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect width="14" height="14" x="8" y="8" rx="2" ry="2"/>
            <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>
          </svg>
        </button>
      {/each}
    </div>
  </div>
</div>

<!-- 输入区域 -->
<div class="pt-4">
  <div class="relative">
    <textarea
      bind:value={messageInput}
      placeholder="在这里输入消息，按 Enter 发送"
      onkeydown={handleKeydown}
      rows="3"
      class="w-full bg-[#f7f7f7] rounded-[20px] border border-[#ebeaea] text-[20px] leading-[1.2] text-[#7e7e7f] px-12 py-4 pr-48 outline-none resize-none"
    ></textarea>
    <button class="absolute left-3 top-1/2 -translate-y-1/2 w-9 h-9 rounded-md border border-[#e5e5e5] bg-white text-[#1e1e1e] flex items-center justify-center" aria-label="添加">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#1e1e1e" stroke-width="2"><path d="M12 5v14m-7-7h14"/></svg>
    </button>
    <div class="absolute right-3 top-1/2 -translate-y-1/2 flex items-center gap-3">
      <div class="relative">
        <button onclick={toggleModelDropdown} class="h-8 px-3 rounded-md border border-[#e5e5e5] text-[20px] leading-[1.2] text-black flex items-center gap-1 bg-white" aria-haspopup="listbox" aria-expanded={showModelDropdown} aria-label="选择模型">
          {selectedModel}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#757575" stroke-width="2"><polyline points="6,9 12,15 18,9"/></svg>
        </button>
        {#if showModelDropdown}
          <div class="absolute bottom-full right-0 mb-1 min-w-40 bg-white border border-[#e5e5e5] rounded-lg shadow-md z-10">
            {#each models as model}
              <button class="w-full px-3 py-2 text-left text-[16px] {model === selectedModel ? 'bg-blue-50 text-blue-600 font-medium' : 'text-black'}" onclick={() => selectModel(model)}>{model}</button>
            {/each}
          </div>
        {/if}
      </div>
      <button class="w-7 h-7 rounded border border-[#e5e5e5] flex items-center justify-center" aria-label="设置">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#1e1e1e" stroke-width="2"><circle cx="12" cy="12" r="3"/></svg>
      </button>
      <button class="w-9 h-9 rounded-full bg-[#1f1f1f] text-white flex items-center justify-center disabled:opacity-50" onclick={sendMessage} disabled={!messageInput.trim()} aria-label="发送">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2"><path d="M22 2L11 13"/><polygon points="22,2 15,22 11,13 2,9 22,2"/></svg>
      </button>
    </div>
  </div>
</div>


