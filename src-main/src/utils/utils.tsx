import { invoke } from "@tauri-apps/api/core";
import { FileList } from "./types";

export const formatFileList = (fileList: FileList) => {
  const formattedList = fileList.map((file) => {
    // remove json extension
    const name = file.file_name.slice(0, -5);
    const date = new Date(file.file_creation_date * 1000);

    return {
      fileName: file.file_name,
      displayName: name,
      date,
      durationInSec: file.macro_duration,
      content: file.file_content,
    };
  });

// Last created item appear first
  const sortedList = formattedList.sort((fileA, fileB) => fileB.date.getTime() - fileA.date.getTime())
  return sortedList;
};

// export async function deleteFile(path: string) {
//   try {
//     await invoke("delete_file", { path });
//     console.log(`Deleted file: ${path}`);
//   } catch (err) {
//     console.error("Failed to delete file:", err);
//   }
// }

export async function deleteFileFromName(fileName: string) {
  const appConfigPath: string = await invoke("get_app_config_path");
  const path = `${appConfigPath}/${fileName}`;

  try {
    await invoke("delete_file", { path });
    console.log(`Deleted file: ${path}`);
  } catch (err) {
    console.error("Failed to delete file:", err);
  }

}

export async function renameFile(oldName: string, newName: string) {
  const appConfigPath: string = await invoke("get_app_config_path");
  const oldPath = `${appConfigPath}/${oldName}`;
  const newPath = `${appConfigPath}/${newName}`;

  try {
    await invoke("rename_file", { oldPath, newPath });
    console.log(`Renamed file: ${oldPath} -> ${newPath}`);
  } catch (err) {
    console.error("Failed to rename file:", err);
  }
}
