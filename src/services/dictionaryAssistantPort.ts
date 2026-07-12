import type { DictionaryAssistantPort } from "../types/llm";

let installedPort: DictionaryAssistantPort | null = null;

/** 注册由宿主实现的授权与调用端口；返回函数用于卸载同一实例。 */
export function installDictionaryAssistantPort(port: DictionaryAssistantPort) {
  installedPort = port;
  return () => {
    if (installedPort === port) installedPort = null;
  };
}

/** 当前仅作为能力发现接口；词典气泡尚未调用。 */
export function getDictionaryAssistantPort() {
  return installedPort;
}
