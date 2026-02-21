#!/usr/bin/env python3
"""将 claude-api (Go) 导出的账号 JSON 转换为 kiro.rs credentials.json 格式"""
import json
import sys

def convert(input_file: str, output_file: str = "credentials.json") -> None:
    with open(input_file, "r", encoding="utf-8") as f:
        accounts = json.load(f)

    credentials = []
    for acc in accounts:
        cred: dict = {"refreshToken": acc["refreshToken"]}

        # 有 clientId/clientSecret 则为 idc 认证，否则为 social
        if acc.get("clientId") and acc.get("clientSecret"):
            cred["authMethod"] = "idc"
            cred["clientId"] = acc["clientId"]
            cred["clientSecret"] = acc["clientSecret"]
        else:
            cred["authMethod"] = "social"

        # label 映射为 email
        if acc.get("label"):
            cred["email"] = acc["label"]

        credentials.append(cred)

    with open(output_file, "w", encoding="utf-8") as f:
        json.dump(credentials, f, indent=2, ensure_ascii=False)

    print(f"转换完成: {len(credentials)} 个凭据 -> {output_file}")

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(f"用法: python3 {sys.argv[0]} <claude-api导出.json> [输出文件]")
        sys.exit(1)
    output = sys.argv[2] if len(sys.argv) > 2 else "credentials.json"
    convert(sys.argv[1], output)
