import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke("greet", { name }));
  }

  async function paste() {
      setGreetMsg(await invoke("paste"))
  }

  return (
    <main className="mx-auto flex min-h-screen w-full max-w-2xl flex-col gap-8 px-6 py-16">
      <header className="space-y-2">
        <h1 className="text-3xl font-semibold tracking-tight">Share Clip</h1>
        <p className="text-sm text-muted-foreground">shadcn/ui + Radix primitives 已接入。</p>
      </header>

      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          greet();
        }}
      >
        <input
          id="greet-input"
          className="h-9 flex-1 rounded-md border border-input bg-background px-3 text-sm"
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter a name..."
        />
        <Button type="submit">Greet</Button>
      </form>

      <p className="text-sm text-muted-foreground">{greetMsg}</p>
        <Button type="submit" onClick={paste}>粘贴</Button>

      <Dialog>
        <DialogTrigger asChild>
          <Button variant="secondary">打开 Radix Dialog 示例</Button>
        </DialogTrigger>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Radix Primitive 已可用</DialogTitle>
            <DialogDescription>
              当前对话框由 `@radix-ui/react-dialog` 构建，样式来自 shadcn/ui 组件模式。
            </DialogDescription>
          </DialogHeader>
        </DialogContent>
      </Dialog>
    </main>
  );
}

export default App;
