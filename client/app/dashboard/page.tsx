"use client";

import { useEffect, useState } from "react";

interface SystemData {
  name: string;
  app_uptime: number;
  host_uptime: number;
  temperature: number;
  weather: string;
  tailscale_ip: string;
}

export default function Page() {
  const [systemData, setSystemData] = useState<SystemData | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function sendPostRequest() {
      try {
        const response = await fetch("/api/v1/cmd", {
          method: "POST", // 指定使用 POST 方法
          headers: {
            "Content-Type": "application/json", // 告知伺服器資料格式
          },
          body: JSON.stringify({ cmd: "p system show" }), // 請求主體資料
        });

        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`);
        }

        const data = await response.json();
        setSystemData(data);
      } catch (err: any) {
        setError(err.message || "發生錯誤");
      }
    }

    // 當元件掛載時自動呼叫 POST 請求
    sendPostRequest();
  }, []); // 空的依賴陣列表示只在元件首次掛載時執行

  const formatUptime = (seconds: number) => {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    return `${days}d ${String(hours).padStart(2, "0")}:${String(
      minutes
    ).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
  };

  return (
    <div className="flex justify-center bg-gray-100">
      <div className="bg-white shadow-md rounded-lg p-6 max-w-xl w-full mx-4 mt-10">
        <h1 className="text-2xl font-bold text-center mb-4">📋 系統資訊</h1>
        {systemData ? (
          <div className="space-y-3">
            <div className="flex justify-between">
              <span className="font-semibold">名稱:</span>
              <span>{systemData.name}</span>
            </div>
            <div className="flex justify-between">
              <span className="font-semibold">APP 運行時間:</span>
              <span>{formatUptime(systemData.app_uptime)}</span>
            </div>
            <div className="flex justify-between">
              <span className="font-semibold">系統運行時間:</span>
              <span>{formatUptime(systemData.host_uptime)}</span>
            </div>
            <div className="flex justify-between">
              <span className="font-semibold">🌡 溫度:</span>
              <span>{systemData.temperature.toFixed(1)}°C</span>
            </div>
            <div className="flex justify-between">
              <span className="font-semibold">Tailscale IP:</span>
              <span>{systemData.tailscale_ip}</span>
            </div>
          </div>
        ) : (
          <p className="text-center text-gray-500">載入中...</p>
        )}
      </div>
    </div>
  );
}
