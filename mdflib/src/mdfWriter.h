
#pragma once
/* mdfWriter.h */

extern int mdfOpen(const char* filename);
extern int mdfCreateChannelGroup(uint32_t recordId, uint32_t recordLen, uint32_t timeChannelSize, double timeChannelConv);
extern int mdfCreateChannel(const char* name, uint8_t msize, int8_t encoding, uint32_t dim, uint32_t byteOffset, double factor, double offset, const char* unit);
extern int mdfWriteHeader();
extern int mdfWriteRecord(const uint8_t* record, uint32_t recordLen);
extern int mdfClose();

