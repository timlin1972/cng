"use client";

import { useState, useEffect } from "react";

export default function Page() {
  const [files, setFiles] = useState<File[]>([]);
  const [uploadStatus, setUploadStatus] = useState<string>("");

  useEffect(() => {
    if (files.length > 0) {
      const uploadFiles = async () => {
        const formData = new FormData();
        files.forEach((file, index) => {
          formData.append(`file-${index}`, file);
        });

        try {
          const response = await fetch("/api/v1/upload", {
            method: "POST",
            body: formData,
          });

          if (response.ok) {
            setUploadStatus("Files uploaded successfully!");
          } else {
            setUploadStatus("Failed to upload files.");
          }
        } catch (error) {
          setUploadStatus("Error occurred during upload.");
        }
      };

      uploadFiles();
    }
  }, [files]);

  const handleFilesChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    if (event.target.files) {
      setFiles(Array.from(event.target.files)); // Convert FileList to an array
      setUploadStatus(""); // Reset status
    }
  };

  return (
    <div className="min-h-screen bg-gray-100 p-6">
      <h1 className="text-3xl font-bold text-center mb-6">Upload Files</h1>
      <div className="max-w-md mx-auto bg-white p-4 rounded shadow-md">
        <label
          htmlFor="fileInput"
          className="block text-sm font-medium text-gray-700 mb-2"
        >
          Select files:
        </label>
        <input
          type="file"
          id="fileInput"
          multiple // Allow multiple file selection
          onChange={handleFilesChange}
          className="block w-full text-sm text-gray-500 border border-gray-300 rounded-lg p-2 mb-4"
        />
        <p className="text-center text-sm text-gray-500 mb-2">
          {uploadStatus || "No files selected."}
        </p>
      </div>
    </div>
  );
}
