"use client";

import { useEffect, useState } from "react";

export default function Page() {
  const [result, setResult] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function sendPostRequest() {
      try {
        const response = await fetch("/api/v1/cmd", {
          method: "POST", // 指定使用 POST 方法
          headers: {
            "Content-Type": "application/json", // 告知伺服器資料格式
          },
          body: JSON.stringify({ cmd: "p todos show" }), // 請求主體資料
        });

        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`);
        }

        const data = await response.json();
        setResult(data);
      } catch (err: any) {
        setError(err.message || "發生錯誤");
      }
    }

    // 當元件掛載時自動呼叫 POST 請求
    sendPostRequest();
  }, []); // 空的依賴陣列表示只在元件首次掛載時執行

  return (
    <div>
      <h1>自動送 POST 請求</h1>
      {error && <p style={{ color: "red" }}>錯誤：{error}</p>}
      {result ? (
        <div>
          <h2>回應資料：</h2>
          <pre>{JSON.stringify(result, null, 2)}</pre>
        </div>
      ) : (
        <p>正在載入...</p>
      )}
    </div>
  );
}
