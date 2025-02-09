"use client";

import { useEffect, useState } from "react";

interface DeviceData {
  ts: number;
  name: string;
  onboard: boolean;
  app_uptime: number;
  host_uptime: number;
  version: string;
  temperature: number;
  last_seen: number;
  tailscale_ip: string;
}

export default function Page() {
  const [devicesData, setDevicesData] = useState<DeviceData[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function sendPostRequest() {
      try {
        const response = await fetch("/api/v1/cmd", {
          method: "POST", // 指定使用 POST 方法
          headers: {
            "Content-Type": "application/json", // 告知伺服器資料格式
          },
          body: JSON.stringify({ cmd: "p devices show" }), // 請求主體資料
        });

        if (!response.ok) {
          throw new Error(`HTTP error! status: ${response.status}`);
        }

        const data = await response.json();
        setDevicesData(data);
      } catch (err: any) {
        setError(err.message || "發生錯誤");
      }
    }

    // 當元件掛載時自動呼叫 POST 請求
    sendPostRequest();
  }, []); // 空的依賴陣列表示只在元件首次掛載時執行

  // 格式化時間戳
  const formatTimestamp = (ts: number) => {
    const date = new Date(ts * 1000);
    return date.toLocaleString("zh-TW", { hour12: false });
  };

  // 格式化 Uptime (秒轉換成 d hh:mm:ss)
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
    <div className="min-h-screen bg-gray-100 p-6">
      <h1 className="text-3xl font-bold text-center mb-6">🖥 設備資訊</h1>
      <div className="overflow-x-auto">
        <table className="min-w-full bg-white shadow-md rounded-lg overflow-hidden">
          {/* 表頭 */}
          <thead className="bg-blue-500 text-white">
            <tr>
              <th className="py-3 px-6 text-left">📍 名稱</th>
              <th className="py-3 px-6 text-left">📅 時間</th>
              <th className="py-3 px-6 text-left">🕒 APP 運行時間</th>
              <th className="py-3 px-6 text-left">🖥 系統運行時間</th>
              <th className="py-3 px-6 text-left">📦 版本</th>
              <th className="py-3 px-6 text-left">🌡 溫度 (°C)</th>
              <th className="py-3 px-6 text-left">🕵️‍♂️ 最後在線</th>
              <th className="py-3 px-6 text-left">🌐 Tailscale IP</th>
            </tr>
          </thead>
          {/* 表格內容 */}
          <tbody>
            {devicesData.map((item, index) => (
              <tr
                key={item.name}
                className={`border-b ${
                  index % 2 === 0 ? "bg-gray-50" : "bg-white"
                }`}
              >
                <td className="py-3 px-6">{item.name}</td>
                <td className="py-3 px-6">{formatTimestamp(item.ts)}</td>
                <td className="py-3 px-6">{formatUptime(item.app_uptime)}</td>
                <td className="py-3 px-6">{formatUptime(item.host_uptime)}</td>
                <td className="py-3 px-6">{item.version}</td>
                <td className="py-3 px-6">{item.temperature.toFixed(1)}</td>
                <td className="py-3 px-6">{formatTimestamp(item.last_seen)}</td>
                <td className="py-3 px-6">{item.tailscale_ip}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
