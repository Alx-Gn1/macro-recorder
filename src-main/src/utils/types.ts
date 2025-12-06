export type File = { file_name: string; file_creation_date: number; macro_duration: number; file_content: string };

export type FileList = Array<File>;

export type FormattedFile = {
  fileName: string;
  displayName: string;
  date: Date;
  durationInSec: number;
  content: string;
};
export type FormattedFileList = Array<FormattedFile>;
